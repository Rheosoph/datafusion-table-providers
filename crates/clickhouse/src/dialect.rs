use std::sync::Arc;

use arrow::array::timezone::Tz;
use chrono::{DateTime, Utc};
use datafusion::arrow::datatypes::TimeUnit;
use datafusion::sql::sqlparser::ast::{self, Ident, ObjectName};
use datafusion::sql::unparser::dialect::{DefaultDialect, Dialect};

/// ClickHouse cannot parse a UTC offset inside a timestamp string, which is how the default
/// dialect renders zoned literals (`'2026-01-03 00:00:00 +00:00'`), so every pushed-down
/// filter on a `DateTime64` column failed.
///
/// Every timestamp is written as UTC wall-clock time and cast to `DateTime64(p, 'UTC')`:
/// zoned values are converted here, and a naive DataFusion timestamp already is an epoch
/// read as UTC. The cast names the same instant whatever zone the column or server uses —
/// it cannot carry the literal's own zone, because the unparser asks for the cast type of
/// a literal without one.
#[derive(Debug, Default)]
pub struct ClickHouseDialect;

impl Dialect for ClickHouseDialect {
    fn identifier_quote_style(&self, identifier: &str) -> Option<char> {
        DefaultDialect {}.identifier_quote_style(identifier)
    }

    fn timestamp_cast_dtype(&self, time_unit: &TimeUnit, _tz: &Option<Arc<str>>) -> ast::DataType {
        let precision = match time_unit {
            TimeUnit::Second => 0,
            TimeUnit::Millisecond => 3,
            TimeUnit::Microsecond => 6,
            TimeUnit::Nanosecond => 9,
        };
        ast::DataType::Custom(
            ObjectName::from(vec![Ident::new("DateTime64")]),
            vec![precision.to_string(), "'UTC'".to_string()],
        )
    }

    fn timestamp_with_tz_to_string(&self, dt: DateTime<Tz>, unit: TimeUnit) -> String {
        let format = match unit {
            TimeUnit::Second => "%Y-%m-%d %H:%M:%S",
            TimeUnit::Millisecond => "%Y-%m-%d %H:%M:%S%.3f",
            TimeUnit::Microsecond => "%Y-%m-%d %H:%M:%S%.6f",
            TimeUnit::Nanosecond => "%Y-%m-%d %H:%M:%S%.9f",
        };
        dt.with_timezone(&Utc).format(format).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use datafusion::common::ScalarValue;
    use datafusion::logical_expr::{col, lit};
    use datafusion::sql::unparser::Unparser;

    /// 2026-01-03 00:00:00 UTC.
    const MICROS: i64 = 1_767_398_400_000_000;

    fn render(value: ScalarValue) -> String {
        Unparser::new(&ClickHouseDialect)
            .expr_to_sql(&col("created_at").gt_eq(lit(value)))
            .unwrap()
            .to_string()
    }

    #[test]
    fn zoned_literals_carry_no_offset_and_cast_to_utc() {
        assert_eq!(
            render(ScalarValue::TimestampMicrosecond(
                Some(MICROS),
                Some("+00:00".into())
            )),
            "(created_at >= CAST('2026-01-03 00:00:00.000000' AS DateTime64(6, 'UTC')))"
        );
    }

    #[test]
    fn zoned_literals_keep_their_instant_across_zones() {
        assert_eq!(
            render(ScalarValue::TimestampSecond(
                Some(MICROS / 1_000_000),
                Some("Europe/Berlin".into())
            )),
            "(created_at >= CAST('2026-01-03 00:00:00' AS DateTime64(0, 'UTC')))"
        );
    }

    #[test]
    fn naive_literals_keep_their_epoch() {
        assert_eq!(
            render(ScalarValue::TimestampMillisecond(
                Some(MICROS / 1_000),
                None
            )),
            "(created_at >= CAST('2026-01-03 00:00:00' AS DateTime64(3, 'UTC')))"
        );
    }
}
