use mago_ast::ast::*;
use mago_fixer::SafetyClassification;
use mago_reporting::*;
use mago_span::*;
use mago_walker::Walker;

use crate::context::LintContext;
use crate::rule::Rule;

#[derive(Clone, Copy, Debug)]
pub struct OptionalParameterBeforeRequiredRule;

impl Rule for OptionalParameterBeforeRequiredRule {
    fn get_name(&self) -> &'static str {
        "optional-parameter-before-required"
    }

    fn get_default_level(&self) -> Option<Level> {
        Some(Level::Warning)
    }
}

impl<'a> Walker<LintContext<'a>> for OptionalParameterBeforeRequiredRule {
    fn walk_function_like_parameter_list(
        &self,
        function_like_parameter_list: &FunctionLikeParameterList,
        context: &mut LintContext<'a>,
    ) {
        let mut optional_parameters = Vec::new();

        for parameter in function_like_parameter_list.parameters.iter() {
            let name = context.lookup(&parameter.variable.name);

            if let Some(default_value) = &parameter.default_value {
                // Store optional parameters along with their spans
                optional_parameters.push((parameter.variable.name, parameter.variable.span(), default_value.span()));
            } else if !optional_parameters.is_empty() {
                // A required parameter follows one or more optional parameters
                let issue = Issue::new(
                    context.level(),
                    format!(
                        "Optional parameter(s) `{}` defined before required parameter `{}`.",
                        optional_parameters
                            .iter()
                            .map(|(opt_name, _, _)| context.lookup(opt_name).to_string())
                            .collect::<Vec<_>>()
                            .join("`, `"),
                        name
                    ),
                )
                .with_annotation(
                    Annotation::primary(parameter.variable.span())
                        .with_message(format!("Required parameter `{}` defined here", name)),
                )
                .with_annotations(optional_parameters.iter().map(|(opt_name, opt_span, _)| {
                    Annotation::secondary(*opt_span)
                        .with_message(format!("Optional parameter `{}` defined here.", context.lookup(opt_name)))
                }))
                .with_note("Parameters after an optional one are implicitly required.")
                .with_note("Defining optional parameters before required ones has been deprecated since PHP 8.0.")
                .with_help("Move all optional parameters to the end of the parameter list to resolve this issue.");

                context.report_with_fix(issue, |plan| {
                    for (_, _, default_span) in &optional_parameters {
                        plan.delete(default_span.to_range(), SafetyClassification::PotentiallyUnsafe);
                    }
                });

                // Clear optional parameters to handle subsequent issues
                optional_parameters.clear();
            }
        }
    }
}
