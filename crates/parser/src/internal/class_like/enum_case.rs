use fennec_ast::ast::*;
use fennec_ast::sequence::Sequence;
use fennec_token::T;

use crate::error::ParseError;
use crate::internal::expression;
use crate::internal::identifier::parse_local_identifier;
use crate::internal::terminator::parse_terminator;
use crate::internal::token_stream::TokenStream;
use crate::internal::utils;

pub fn parse_enum_case_with_attributes(
    stream: &mut TokenStream<'_, '_>,
    attributes: Sequence<AttributeList>,
) -> Result<EnumCase, ParseError> {
    Ok(EnumCase {
        attributes,
        case: utils::expect_keyword(stream, T!["case"])?,
        item: parse_enum_case_item(stream)?,
        terminator: parse_terminator(stream)?,
    })
}

pub fn parse_enum_case_item(stream: &mut TokenStream<'_, '_>) -> Result<EnumCaseItem, ParseError> {
    let name = parse_local_identifier(stream)?;

    Ok(match utils::maybe_peek(stream)?.map(|t| t.kind) {
        Some(T!["="]) => {
            let equals = utils::expect_span(stream, T!["="])?;
            let value = expression::parse_expression(stream)?;

            EnumCaseItem::Backed(EnumCaseBackedItem { name, equals, value })
        }
        _ => EnumCaseItem::Unit(EnumCaseUnitItem { name }),
    })
}
