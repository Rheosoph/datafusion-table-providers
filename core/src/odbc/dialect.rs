use datafusion::sql::sqlparser::ast;
use datafusion::sql::unparser::dialect::{DefaultDialect, Dialect};

/// A generic ODBC source can be any database, so pushed-down SQL keeps to SQL-92 spellings
/// where the default dialect uses engine-specific ones: `DOUBLE` (emitted for every
/// Float64 cast, e.g. `SUM` over a float column) is rejected by PostgreSQL, Oracle and
/// SQL Server, while `DOUBLE PRECISION` is accepted by all of them, Snowflake included.
#[derive(Debug, Default)]
pub struct OdbcDialect;

impl Dialect for OdbcDialect {
    fn identifier_quote_style(&self, identifier: &str) -> Option<char> {
        DefaultDialect {}.identifier_quote_style(identifier)
    }

    fn float64_ast_dtype(&self) -> ast::DataType {
        ast::DataType::DoublePrecision
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use datafusion::arrow::datatypes::DataType;
    use datafusion::logical_expr::{cast, col};
    use datafusion::sql::unparser::Unparser;

    #[test]
    fn float64_casts_use_the_sql92_spelling() {
        let sql = Unparser::new(&OdbcDialect)
            .expr_to_sql(&cast(col("score"), DataType::Float64))
            .unwrap()
            .to_string();
        assert_eq!(sql, "CAST(score AS DOUBLE PRECISION)");
    }

    #[test]
    fn identifiers_are_quoted_like_the_default_dialect() {
        let sql = Unparser::new(&OdbcDialect)
            .expr_to_sql(&col("Mixed Case"))
            .unwrap()
            .to_string();
        assert_eq!(sql, "\"Mixed Case\"");
    }
}
