use mago_ast::*;
use mago_reporting::*;
use mago_walker::Walker;

use crate::context::LintContext;
use crate::rule::Rule;

const REQUEST_VARIABLE: &str = "$_REQUEST";

#[derive(Clone, Debug)]
pub struct NoRequestVariableRule;

impl Rule for NoRequestVariableRule {
    fn get_name(&self) -> &'static str {
        "no-request-variable"
    }

    fn get_default_level(&self) -> Option<Level> {
        Some(Level::Error)
    }
}

impl<'a> Walker<LintContext<'a>> for NoRequestVariableRule {
    fn walk_in_direct_variable<'ast>(&self, direct_variable: &'ast DirectVariable, context: &mut LintContext<'a>) {
        let name = context.interner.lookup(&direct_variable.name);
        if !REQUEST_VARIABLE.eq(name) {
            return;
        }

        let issue = Issue::new(context.level(), "Unsafe use of `$_REQUEST` variable.")
            .with_annotation(
                Annotation::primary(direct_variable.span).with_message("The `$_REQUEST` variable is used here."),
            )
            .with_help("use `$_GET`, `$_POST`, or `$_COOKIE` instead for better clarity.");

        context.report(issue);
    }
}
