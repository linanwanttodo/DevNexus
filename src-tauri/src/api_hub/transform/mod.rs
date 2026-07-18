pub mod anthropic;
pub mod gemini;
pub mod openai;
pub mod responses;

// 向后兼容：server 等模块可继续通过 transform::openai::* 访问
// Anthropic 转换已迁移到 anthropic 模块，openai 模块 re-export
