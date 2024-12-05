use fennec_ast::*;
use fennec_span::*;

use crate::comment::CommentFlags;
use crate::document::*;
use crate::format::call_node::CallLikeNode;
use crate::format::misc::should_hug_expression;
use crate::utils::will_break;
use crate::Format;
use crate::Formatter;

use super::misc::is_string_word_type;

pub(super) fn print_call_arguments<'a>(f: &mut Formatter<'a>, expression: &CallLikeNode<'a>) -> Document<'a> {
    let Some(argument_list) = expression.arguments() else {
        return Document::empty();
    };

    print_argument_list(f, argument_list)
}

pub(super) fn print_argument_list<'a>(f: &mut Formatter<'a>, argument_list: &'a ArgumentList) -> Document<'a> {
    let mut contents = vec![Document::String("(")];
    if argument_list.arguments.is_empty() {
        contents.extend(f.print_inner_comment(argument_list.span()));
        contents.push(Document::String(")"));

        return Document::Array(contents);
    }

    let get_printed_arguments = |p: &mut Formatter<'a>, skip_index: isize| {
        let mut printed_arguments = vec![];
        let mut length = argument_list.arguments.len();
        let arguments: Box<dyn Iterator<Item = (usize, &'a Argument)>> = match skip_index {
            _ if skip_index > 0 => {
                length -= skip_index as usize;
                Box::new(argument_list.arguments.iter().skip(skip_index as usize).enumerate())
            }
            _ if skip_index < 0 => {
                length -= (-skip_index) as usize;
                Box::new(
                    argument_list
                        .arguments
                        .iter()
                        .take(argument_list.arguments.len() - (-skip_index) as usize)
                        .enumerate(),
                )
            }
            _ => Box::new(argument_list.arguments.iter().enumerate()),
        };

        for (i, element) in arguments {
            let mut argument = vec![element.format(p)];
            if i < length - 1 {
                argument.push(Document::String(","));

                if p.is_next_line_empty(element.span()) {
                    argument.push(Document::Line(Line::hardline()));
                    argument.push(Document::Line(Line::hardline()));
                    argument.push(Document::BreakParent);
                } else {
                    argument.push(Document::Line(Line::default()));
                }
            }

            printed_arguments.push(Document::Array(argument));
        }

        printed_arguments
    };

    let all_arguments_broken_out = |f: &mut Formatter<'a>| {
        let mut parts = vec![];
        parts.push(Document::String("("));
        parts.push(Document::Indent(vec![
            Document::Line(Line::default()),
            Document::Array(get_printed_arguments(f, 0)),
            if f.settings.trailing_comma { Document::String(",") } else { Document::empty() },
        ]));

        parts.push(Document::Line(Line::default()));
        parts.push(Document::String(")"));

        Document::Group(Group::new(parts).with_break(true))
    };

    if should_inline_single_breaking_argument(f, argument_list) {
        // we have a single argument that we can hug
        // this means we can avoid any spacing and just print the argument
        // between the parentheses
        let single_argument = argument_list.arguments.first().unwrap().format(f);

        return Document::Group(Group::new(vec![
            Document::String("("),
            Document::Group(Group::new(vec![single_argument]).with_break(true)),
            Document::String(")"),
        ]));
    }

    if should_expand_first_arg(f, argument_list) {
        f.argument_state.expand_first_argument = true;
        let mut first_doc = argument_list.arguments.first().unwrap().format(f);
        f.argument_state.expand_first_argument = false;

        if will_break(&mut first_doc) {
            let last_doc = get_printed_arguments(f, 1).pop().unwrap();

            return Document::Array(vec![
                Document::BreakParent,
                Document::Group(Group::new_conditional_group(
                    vec![
                        Document::String("("),
                        Document::Group(Group::new(vec![first_doc]).with_break(true)),
                        Document::String(", "),
                        last_doc,
                        Document::String(")"),
                    ],
                    vec![all_arguments_broken_out(f)],
                )),
            ]);
        }
    }

    if should_expand_last_arg(f, argument_list) {
        let mut printed_arguments = get_printed_arguments(f, -1);
        if printed_arguments.iter_mut().any(will_break) {
            return all_arguments_broken_out(f);
        }

        if !printed_arguments.is_empty() {
            printed_arguments.push(Document::String(","));
            printed_arguments.push(Document::Line(Line::default()));
        }

        let get_last_doc = |p: &mut Formatter<'a>| {
            p.argument_state.expand_last_argument = true;
            let last_doc = argument_list.arguments.last().unwrap().format(p);
            p.argument_state.expand_last_argument = false;

            last_doc
        };

        let mut last_doc = get_last_doc(f);

        if will_break(&mut last_doc) {
            return Document::Array(vec![
                Document::BreakParent,
                Document::Group(Group::new_conditional_group(
                    vec![
                        Document::String("("),
                        Document::Array(printed_arguments),
                        Document::Group(Group::new(vec![last_doc]).with_break(true)),
                        Document::String(")"),
                    ],
                    vec![all_arguments_broken_out(f)],
                )),
            ]);
        }

        return Document::Group(Group::new_conditional_group(
            vec![Document::String("("), Document::Array(printed_arguments), last_doc, Document::String(")")],
            vec![
                Document::Array(vec![
                    Document::String("("),
                    Document::Array(get_printed_arguments(f, -1)),
                    Document::String(","),
                    Document::Line(Line::default()),
                    Document::Group(Group::new(vec![get_last_doc(f)]).with_break(true)),
                    Document::String(")"),
                ]),
                all_arguments_broken_out(f),
            ],
        ));
    }

    let mut printed_arguments = get_printed_arguments(f, 0);

    printed_arguments.insert(0, Document::Line(Line::softline()));
    contents.push(Document::Indent(printed_arguments));
    if f.settings.trailing_comma {
        contents.push(Document::IfBreak(IfBreak::then(Document::String(","))));
    }
    contents.push(Document::Line(Line::softline()));
    contents.push(Document::String(")"));

    Document::Group(Group::new(contents))
}

fn should_inline_single_breaking_argument<'a>(f: &Formatter<'a>, argument_list: &'a ArgumentList) -> bool {
    if !f.settings.inline_single_breaking_argument {
        return false;
    }

    if argument_list.arguments.len() != 1 {
        return false;
    }

    let argument = &argument_list.arguments.as_slice()[0];
    if f.has_comment(argument.span(), CommentFlags::Leading | CommentFlags::Trailing)
        || f.has_comment(argument.span(), CommentFlags::Leading | CommentFlags::Trailing)
    {
        return false;
    }

    should_hug_expression(f, argument.value())
}

/// * Reference <https://github.com/prettier/prettier/blob/3.3.3/src/language-js/print/call-arguments.js#L247-L272>
fn should_expand_first_arg<'a>(f: &Formatter<'a>, argument_list: &'a ArgumentList) -> bool {
    if argument_list.arguments.len() != 2 {
        return false;
    }

    let arguments = argument_list.arguments.as_slice();
    let first_argument = &arguments[0];
    let second_argument = &arguments[1];

    if f.has_comment(first_argument.span(), CommentFlags::Leading | CommentFlags::Trailing)
        || f.has_comment(second_argument.span(), CommentFlags::Leading | CommentFlags::Trailing)
    {
        return false;
    }

    match first_argument.value() {
        Expression::Closure(c) if c.use_clause.is_none() => {}
        Expression::Match(_) => {}
        _ => return false,
    };

    match second_argument.value() {
        Expression::Array(_) | Expression::List(_) | Expression::LegacyArray(_) => false,
        Expression::Closure(_) | Expression::ArrowFunction(_) | Expression::TernaryOperation(_) => false,
        expression => is_hopefully_short_call_argument(expression) && !could_expand_argument_value(expression, false),
    }
}

/// * Reference <https://github.com/prettier/prettier/blob/52829385bcc4d785e58ae2602c0b098a643523c9/src/language-js/print/call-arguments.js#L234-L258>
fn should_expand_last_arg<'a>(f: &Formatter<'a>, argument_list: &'a ArgumentList) -> bool {
    let Some(last_argument) = argument_list.arguments.last() else { return false };
    if f.has_comment(last_argument.span(), CommentFlags::Leading | CommentFlags::Trailing) {
        return false;
    }

    let last_argument_value = last_argument.value();

    let penultimate_argument = if argument_list.arguments.len() >= 2 {
        argument_list.arguments.get(argument_list.arguments.len() - 2)
    } else {
        None
    };

    let penultimate_argument_comments = penultimate_argument
        .map(|a| f.has_comment(a.span(), CommentFlags::Leading | CommentFlags::Trailing))
        .unwrap_or(false);

    could_expand_argument_value(last_argument_value, false)
        // If the last two arguments are of the same type,
        // disable last element expansion.
        && (penultimate_argument.is_none()
            || penultimate_argument_comments
            || matches!(penultimate_argument, Some(argument) if argument.value().node_kind() != last_argument_value.node_kind()))
        && (argument_list.arguments.len() != 2
            ||penultimate_argument_comments
            || !matches!(last_argument_value, Expression::Array(_) | Expression::LegacyArray(_))
            || !matches!(penultimate_argument.map(|a| a.value()), Some(Expression::Closure(c)) if c.use_clause.is_none()))
        && (argument_list.arguments.len() != 2
            || penultimate_argument_comments
            || !matches!(penultimate_argument.map(|a| a.value()), Some(Expression::Array(_) | Expression::LegacyArray(_)))
            || !matches!(last_argument_value, Expression::Closure(c) if c.use_clause.is_none()))
}

fn is_hopefully_short_call_argument(mut node: &Expression) -> bool {
    loop {
        node = match node {
            Expression::Parenthesized(parenthesized) => &parenthesized.expression,
            Expression::Suppressed(suppressed) => &suppressed.expression,
            Expression::Referenced(referenced) => &referenced.expression,
            _ => break,
        };
    }

    match node {
        Expression::Call(call) => {
            let argument_list = match call {
                Call::Function(function_call) => &function_call.arguments,
                Call::Method(method_call) => &method_call.arguments,
                Call::NullSafeMethod(null_safe_method_call) => &null_safe_method_call.arguments,
                Call::StaticMethod(static_method_call) => &static_method_call.arguments,
            };

            argument_list.arguments.len() < 2
        }
        Expression::Instantiation(instantiation) => {
            instantiation.arguments.as_ref().map_or(true, |argument_list| argument_list.arguments.len() < 2)
        }
        node @ Expression::ArithmeticOperation(arithmetic) => {
            if let ArithmeticOperation::Infix(operation) = arithmetic.as_ref() {
                is_simple_call_argument(&operation.lhs, 1) && is_simple_call_argument(&operation.rhs, 1)
            } else {
                is_simple_call_argument(node, 2)
            }
        }
        Expression::BitwiseOperation(bitwise) => {
            if let BitwiseOperation::Infix(operation) = bitwise.as_ref() {
                is_simple_call_argument(&operation.lhs, 1) && is_simple_call_argument(&operation.rhs, 1)
            } else {
                is_simple_call_argument(node, 2)
            }
        }
        Expression::LogicalOperation(logical) => {
            if let LogicalOperation::Infix(operation) = logical.as_ref() {
                is_simple_call_argument(&operation.lhs, 1) && is_simple_call_argument(&operation.rhs, 1)
            } else {
                is_simple_call_argument(node, 2)
            }
        }
        Expression::ComparisonOperation(operation) => {
            is_simple_call_argument(&operation.lhs, 1) && is_simple_call_argument(&operation.rhs, 1)
        }
        Expression::ConcatOperation(operation) => {
            is_simple_call_argument(&operation.lhs, 1) && is_simple_call_argument(&operation.rhs, 1)
        }
        Expression::CoalesceOperation(operation) => {
            is_simple_call_argument(&operation.lhs, 1) && is_simple_call_argument(&operation.rhs, 1)
        }
        _ => is_simple_call_argument(node, 2),
    }
}

fn is_simple_call_argument<'a>(node: &'a Expression, depth: usize) -> bool {
    let is_child_simple = |node: &'a Expression| {
        if depth <= 1 {
            return false;
        }

        is_simple_call_argument(node, depth - 1)
    };

    let is_simple_element = |node: &'a ArrayElement| match node {
        ArrayElement::KeyValue(element) => is_child_simple(&element.key) && is_child_simple(&element.value),
        ArrayElement::Value(element) => is_child_simple(&element.value),
        ArrayElement::Variadic(element) => is_child_simple(&element.value),
        ArrayElement::Missing(_) => true,
    };

    if node.is_literal() || is_string_word_type(node) {
        return true;
    }

    match node {
        Expression::Parenthesized(parenthesized) => is_simple_call_argument(&parenthesized.expression, depth),
        Expression::Suppressed(suppressed) => is_simple_call_argument(&suppressed.expression, depth),
        Expression::Referenced(referenced) => is_simple_call_argument(&referenced.expression, depth),
        Expression::Array(array) => array.elements.iter().all(is_simple_element),
        Expression::LegacyArray(array) => array.elements.iter().all(is_simple_element),
        Expression::Call(call) => {
            let argument_list = match call {
                Call::Function(function_call) => {
                    if !is_simple_call_argument(&function_call.function, depth) {
                        return false;
                    }

                    &function_call.arguments
                }
                Call::Method(method_call) => {
                    if !is_simple_call_argument(&method_call.object, depth) {
                        return false;
                    }

                    &method_call.arguments
                }
                Call::NullSafeMethod(null_safe_method_call) => {
                    if !is_simple_call_argument(&null_safe_method_call.object, depth) {
                        return false;
                    }

                    &null_safe_method_call.arguments
                }
                Call::StaticMethod(static_method_call) => {
                    if !is_simple_call_argument(&static_method_call.class, depth) {
                        return false;
                    }

                    &static_method_call.arguments
                }
            };

            argument_list.arguments.len() <= depth
                && argument_list.arguments.iter().map(|a| a.value()).all(is_child_simple)
        }
        Expression::Access(access) => {
            let object_or_class = match access.as_ref() {
                Access::Property(property_access) => &property_access.object,
                Access::NullSafeProperty(null_safe_property_access) => &null_safe_property_access.object,
                Access::StaticProperty(static_property_access) => &static_property_access.class,
                Access::ClassConstant(class_constant_access) => &class_constant_access.class,
            };

            is_simple_call_argument(object_or_class, depth)
        }
        Expression::ArrayAccess(array_access) => {
            is_simple_call_argument(&array_access.array, depth) && is_simple_call_argument(&array_access.index, depth)
        }
        Expression::Instantiation(instantiation) => {
            if is_simple_call_argument(&instantiation.class, depth) {
                match &instantiation.arguments {
                    Some(argument_list) => {
                        argument_list.arguments.len() <= depth
                            && argument_list.arguments.iter().map(|a| a.value()).all(is_child_simple)
                    }
                    None => true,
                }
            } else {
                false
            }
        }
        Expression::ArithmeticOperation(arithmetic) => {
            if let ArithmeticOperation::Prefix(ArithmeticPrefixOperation {
                operator: ArithmeticPrefixOperator::Minus(_) | ArithmeticPrefixOperator::Plus(_),
                value,
            }) = arithmetic.as_ref()
            {
                is_simple_call_argument(&value, depth)
            } else {
                false
            }
        }
        Expression::BitwiseOperation(bitwise) => {
            if let BitwiseOperation::Prefix(operation) = bitwise.as_ref() {
                is_simple_call_argument(&operation.value, depth)
            } else {
                false
            }
        }
        Expression::LogicalOperation(logical) => {
            if let LogicalOperation::Prefix(operation) = logical.as_ref() {
                is_simple_call_argument(&operation.value, depth)
            } else {
                false
            }
        }
        _ => false,
    }
}

fn could_expand_argument_value<'a>(argument_value: &'a Expression, arrow_chain_recursion: bool) -> bool {
    match argument_value {
        Expression::Array(expr) => expr.elements.len() > 0,
        Expression::LegacyArray(expr) => expr.elements.len() > 0,
        Expression::List(expr) => expr.elements.len() > 0,
        Expression::Closure(_) => true,
        Expression::ArithmeticOperation(arithmetic) => {
            if let ArithmeticOperation::Infix(operation) = arithmetic.as_ref() {
                could_expand_argument_value(&operation.lhs, arrow_chain_recursion)
            } else {
                false
            }
        }
        Expression::BitwiseOperation(bitwise) => {
            if let BitwiseOperation::Infix(operation) = bitwise.as_ref() {
                could_expand_argument_value(&operation.lhs, arrow_chain_recursion)
            } else {
                false
            }
        }
        Expression::LogicalOperation(logical) => {
            if let LogicalOperation::Infix(operation) = logical.as_ref() {
                could_expand_argument_value(&operation.lhs, arrow_chain_recursion)
            } else {
                false
            }
        }
        Expression::ComparisonOperation(operation) => {
            could_expand_argument_value(&operation.lhs, arrow_chain_recursion)
        }
        Expression::ConcatOperation(operation) => could_expand_argument_value(&operation.lhs, arrow_chain_recursion),
        Expression::CoalesceOperation(operation) => could_expand_argument_value(&operation.lhs, arrow_chain_recursion),
        Expression::ArrowFunction(arrow_function) => match &arrow_function.expression {
            Expression::Array(_) | Expression::List(_) | Expression::LegacyArray(_) => {
                could_expand_argument_value(&arrow_function.expression, true)
            }
            Expression::Call(_) | Expression::TernaryOperation(_) => !arrow_chain_recursion,
            _ => false,
        },
        _ => false,
    }
}