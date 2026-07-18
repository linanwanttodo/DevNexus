//! 兼容别名：历史代码可能 `use transform::openai::*`。
//! Anthropic 互转实现在 `anthropic` 模块。

#[allow(unused_imports)]
pub use super::anthropic::{
    anthropic_to_openai, anthropic_to_openai_req, openai_response_to_anthropic, openai_to_anthropic,
};
