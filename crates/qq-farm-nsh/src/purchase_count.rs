//! 购买数量上限：取限购剩余和按余额可买数量的较小值。

pub(crate) struct PurchaseCost {
    pub count: i64,
    pub balance: Option<i64>,
}

/// 每种有效费用按余额除以单价向下取整，再取最小。没有费用参与约束时返回 `None`。
pub(crate) fn affordable_count(costs: &[PurchaseCost]) -> Option<i64> {
    let mut max: Option<i64> = None;
    for cost in costs {
        if cost.count <= 0 {
            continue;
        }
        let Some(balance) = cost.balance.filter(|balance| *balance >= 0) else {
            continue;
        };
        let can = balance / cost.count;
        max = Some(max.map_or(can, |current| current.min(can)));
    }
    max
}

/// 奶茶兑换上限。没有限购时按 9999，余额未知的费用不参与约束；不够一份时返回 0。
pub(crate) fn exchange_max(limit_remaining: Option<i64>, costs: &[PurchaseCost]) -> i64 {
    let by_balance = affordable_count(costs).unwrap_or(i64::MAX);
    let limit = limit_remaining.unwrap_or(9999);
    9999.min(limit).min(by_balance).max(0)
}

/// 数量选择上限。余额不够一份或限购已用完时仍返回 1，提交时再由余额校验拦住。
pub(crate) fn purchase_max_count(
    limit_remaining: Option<i64>,
    costs: &[PurchaseCost],
    cap: i64,
) -> i64 {
    let cap = if cap > 0 { cap } else { 9999 };
    let limit = limit_remaining.map(|remaining| remaining.max(0)).unwrap_or(cap);
    let affordable = affordable_count(costs).unwrap_or(i64::MAX);
    1.max(cap.min(limit).min(affordable))
}

#[cfg(test)]
mod tests {
    use super::{affordable_count, exchange_max, purchase_max_count, PurchaseCost};

    fn cost(count: i64, balance: Option<i64>) -> PurchaseCost {
        PurchaseCost { count, balance }
    }

    #[test]
    fn affordable_count_uses_the_smallest_whole_purchase() {
        assert_eq!(affordable_count(&[cost(10, Some(100)), cost(1, Some(3))]), Some(3));
        assert_eq!(affordable_count(&[cost(20, Some(100))]), Some(5));
        assert_eq!(affordable_count(&[cost(20, Some(99))]), Some(4));
        assert_eq!(affordable_count(&[cost(20, Some(19))]), Some(0));
        assert_eq!(affordable_count(&[cost(20, Some(0))]), Some(0));
    }

    #[test]
    fn unknown_or_invalid_costs_do_not_limit_the_count() {
        assert_eq!(affordable_count(&[cost(10, Some(50)), cost(1, None)]), Some(5));
        assert_eq!(affordable_count(&[cost(10, Some(50)), cost(0, Some(1))]), Some(5));
        assert_eq!(affordable_count(&[]), None);
        assert_eq!(affordable_count(&[cost(0, None)]), None);
        assert_eq!(affordable_count(&[cost(10, None)]), None);
    }

    #[test]
    fn purchase_max_count_uses_limit_balance_and_cap() {
        assert_eq!(purchase_max_count(Some(10), &[cost(10, Some(35))], 9999), 3);
        assert_eq!(purchase_max_count(Some(4), &[cost(10, Some(1000))], 9999), 4);
        assert_eq!(purchase_max_count(None, &[cost(10, Some(75))], 9999), 7);
        assert_eq!(purchase_max_count(None, &[], 9999), 9999);
        assert_eq!(purchase_max_count(Some(10), &[], 9999), 10);
        assert_eq!(
            purchase_max_count(Some(10), &[cost(10, Some(100)), cost(3, Some(20))], 9999),
            6
        );
        assert_eq!(purchase_max_count(Some(10), &[cost(100, Some(5))], 9999), 1);
        assert_eq!(purchase_max_count(Some(50), &[], 20), 20);
        assert_eq!(purchase_max_count(Some(0), &[cost(1, Some(100))], 9999), 1);
    }

    #[test]
    fn exchange_max_uses_remaining_and_balance() {
        assert_eq!(exchange_max(Some(2), &[cost(10, Some(35))]), 2);
        assert_eq!(exchange_max(None, &[cost(10, Some(35))]), 3);
        assert_eq!(exchange_max(Some(4), &[cost(5, None)]), 4);
        assert_eq!(exchange_max(Some(10), &[cost(100, Some(5))]), 0);
        assert_eq!(exchange_max(Some(0), &[cost(1, Some(100))]), 0);
    }
}
