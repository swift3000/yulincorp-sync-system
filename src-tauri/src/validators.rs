//! 共享输入验证模块 — 所有命令统一使用的验证函数
//!
//! 错误消息遵循三段式：错误原因 → 具体说明 → 可操作建议

use regex::Regex;

// ── 字符串验证 ────────────────────────────────────────

/// 验证字符串非空，返回自定义错误消息
pub fn validate_non_empty(value: &str, field_name: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("{}不能为空，请输入有效的{}", field_name, field_name))
    } else {
        Ok(())
    }
}

/// 验证字符串长度不超过上限
pub fn validate_max_len(value: &str, max: usize, field_name: &str) -> Result<(), String> {
    if value.len() > max {
        Err(format!(
            "{}长度不能超过{}字符，当前{}字符，请缩短后重试",
            field_name,
            max,
            value.len()
        ))
    } else {
        Ok(())
    }
}

/// 验证 Option<String> 非空（如果 Some）
pub fn validate_option_non_empty(value: &Option<String>, field_name: &str) -> Result<(), String> {
    if let Some(v) = value {
        validate_non_empty(v, field_name)
    } else {
        Ok(())
    }
}

/// 验证 Option<String> 长度上限（如果 Some）
pub fn validate_option_max_len(
    value: &Option<String>,
    max: usize,
    field_name: &str,
) -> Result<(), String> {
    if let Some(v) = value {
        validate_max_len(v, max, field_name)
    } else {
        Ok(())
    }
}

/// 验证搜索关键词长度
pub fn validate_keyword(value: &Option<String>) -> Result<(), String> {
    if let Some(kw) = value {
        if kw.len() > 100 {
            return Err("搜索关键词不能超过100个字符".into());
        }
    }
    Ok(())
}

// ── 数值验证 ──────────────────────────────────────────

/// 验证 i64 > 0
pub fn validate_positive_i64(value: i64, field_name: &str) -> Result<(), String> {
    if value <= 0 {
        Err(format!(
            "{}必须大于0，当前值为{}，请检查后重试",
            field_name, value
        ))
    } else {
        Ok(())
    }
}

/// 验证 Option<i64> 大于0（如果 Some）
pub fn validate_option_positive_i64(
    value: &Option<i64>,
    field_name: &str,
) -> Result<(), String> {
    if let Some(v) = value {
        validate_positive_i64(*v, field_name)
    } else {
        Ok(())
    }
}

/// 验证 f64 > 0
pub fn validate_positive_f64(value: f64, field_name: &str) -> Result<(), String> {
    if value <= 0.0 {
        Err(format!(
            "{}必须大于0，当前值为{}，请检查后重试",
            field_name, value
        ))
    } else {
        Ok(())
    }
}

/// 验证 f64 >= 0
pub fn validate_non_negative_f64(value: f64, field_name: &str) -> Result<(), String> {
    if value < 0.0 {
        Err(format!(
            "{}不能为负数，当前值为{}，请检查后重试",
            field_name, value
        ))
    } else {
        Ok(())
    }
}

/// 验证 Option<f64> >= 0（如果 Some）
pub fn validate_option_non_negative_f64(
    value: &Option<f64>,
    field_name: &str,
) -> Result<(), String> {
    if let Some(v) = value {
        validate_non_negative_f64(*v, field_name)
    } else {
        Ok(())
    }
}

/// 验证 i64 >= 0
pub fn validate_non_negative_i64(value: i64, field_name: &str) -> Result<(), String> {
    if value < 0 {
        Err(format!(
            "{}不能为负数，当前值为{}，请检查后重试",
            field_name, value
        ))
    } else {
        Ok(())
    }
}

// ── 日期验证 ──────────────────────────────────────────

/// 验证 YYYY-MM-DD 日期格式
pub fn validate_date_format(value: &str, field_name: &str) -> Result<(), String> {
    let re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
    if !re.is_match(value) {
        Err(format!(
            "{}格式错误（应为 YYYY-MM-DD），当前值为{}，请输入格式正确的日期",
            field_name, value
        ))
    } else {
        Ok(())
    }
}

/// 验证日期不为空且格式正确
pub fn validate_date(value: &str, field_name: &str) -> Result<(), String> {
    validate_non_empty(value, field_name)?;
    validate_date_format(value, field_name)
}

/// 验证日期范围：start_date <= end_date
pub fn validate_date_range(
    start_date: &str,
    end_date: &str,
) -> Result<(), String> {
    if start_date > end_date {
        Err(format!(
            "开始日期({})不能晚于结束日期({})，请调整日期范围",
            start_date, end_date
        ))
    } else {
        Ok(())
    }
}

/// 验证 Option<String> 日期格式（如果 Some）
pub fn validate_option_date(
    value: &Option<String>,
    field_name: &str,
) -> Result<(), String> {
    if let Some(v) = value {
        if !v.is_empty() {
            validate_date_format(v, field_name)?;
        }
    }
    Ok(())
}

// ── 枚举验证 ──────────────────────────────────────────

/// 验证角色值
pub fn validate_role(role: &str) -> Result<(), String> {
    match role {
        "employee" | "boss" | "admin" => Ok(()),
        _ => Err(format!(
            "角色值无效：{}，有效值为 employee（普通员工）、boss（老板）、admin（管理员）",
            role
        )),
    }
}

/// 验证合同方向
pub fn validate_direction(direction: &str) -> Result<(), String> {
    match direction {
        "purchase" | "sales" => Ok(()),
        _ => Err(format!(
            "合同方向无效：{}，有效值为 purchase（采购）、sales（销售）",
            direction
        )),
    }
}

/// 验证交易类型
pub fn validate_transaction_type(t: &str) -> Result<(), String> {
    match t {
        "receipt" | "payment" => Ok(()),
        _ => Err(format!(
            "交易类型无效：{}，有效值为 receipt（收款）、payment（付款）",
            t
        )),
    }
}

/// 验证交易对象类型
pub fn validate_party_type(p: &str) -> Result<(), String> {
    match p {
        "customer" | "supplier" => Ok(()),
        _ => Err(format!(
            "交易对象类型无效：{}，有效值为 customer（客户）、supplier（供应商）",
            p
        )),
    }
}

/// 验证付款方式
pub fn validate_payment_method(method: &str) -> Result<(), String> {
    match method {
        "cash" | "bank_transfer" | "check" | "online" | "other" => Ok(()),
        _ => Err(format!(
            "付款方式无效：{}，有效值为 cash（现金）、bank_transfer（银行转账）、check（支票）、online（在线支付）、other（其他）",
            method
        )),
    }
}

/// 验证字符串是否在合法值列表中
pub fn validate_enum(value: &str, valid_values: &[&str], field_name: &str) -> Result<(), String> {
    if valid_values.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "{}的值无效：{}，有效值为：{}",
            field_name,
            value,
            valid_values.join("、")
        ))
    }
}

// ── 组合验证 ──────────────────────────────────────────

/// 验证字符串：非空 + 长度上限
pub fn validate_string(value: &str, field_name: &str, max_len: usize) -> Result<(), String> {
    validate_non_empty(value, field_name)?;
    validate_max_len(value, max_len, field_name)?;
    Ok(())
}

/// 验证 Option<String>：如果 Some，则非空 + 长度上限
pub fn validate_option_string(
    value: &Option<String>,
    field_name: &str,
    max_len: usize,
) -> Result<(), String> {
    if let Some(v) = value {
        validate_string(v, field_name, max_len)?;
    }
    Ok(())
}
