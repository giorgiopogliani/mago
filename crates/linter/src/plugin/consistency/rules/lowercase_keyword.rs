use mago_ast::ast::*;
use mago_fixer::SafetyClassification;
use mago_reporting::*;
use mago_span::*;
use mago_walker::Walker;

use crate::context::LintContext;
use crate::rule::Rule;

#[derive(Clone, Copy, Debug)]
pub struct LowercaseKeywordRule;

impl Rule for LowercaseKeywordRule {
    fn get_name(&self) -> &'static str {
        "lowercase-keyword"
    }

    fn get_default_level(&self) -> Option<Level> {
        Some(Level::Help)
    }
}

impl<'a> Walker<LintContext<'a>> for LowercaseKeywordRule {
    fn walk_in_keyword<'ast>(&self, keyword: &'ast Keyword, context: &mut LintContext<'a>) {
        let name = context.lookup(&keyword.value);
        let lowered = name.to_ascii_lowercase();
        if !lowered.eq(&name) {
            let issue = Issue::new(context.level(), format!("Keyword `{}` should be in lowercase.", name))
                .with_annotation(Annotation::primary(keyword.span()))
                .with_note(format!("The keyword `{}` does not follow lowercase convention.", name))
                .with_help(format!("Consider using `{}` instead of `{}`.", lowered, name));

            context.report_with_fix(issue, |p| p.replace(keyword.span.to_range(), lowered, SafetyClassification::Safe));
        }
    }
}
