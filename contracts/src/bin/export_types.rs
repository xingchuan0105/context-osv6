//! Export internally-tagged enums that typeshare cannot represent.
use contracts::chat::AnswerBlock;
use ts_rs::TS;

fn main() {
    AnswerBlock::export().expect("export AnswerBlock");
}
