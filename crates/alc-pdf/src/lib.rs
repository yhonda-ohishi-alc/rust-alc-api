//! 拘束時間管理表 PDF 生成ライブラリ
//!
//! レポート型定義 + printpdf ベースの PDF レンダリング

#[cfg(test)]
#[macro_use]
mod test_macros;

pub mod types;

mod render;
pub use render::generate_pdf;
