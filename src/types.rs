#[derive(Debug, Clone)]
pub enum SeppoExpr {
    Number(i64),
    String(String),
    Variable(String),
    Operation(String, Box<SeppoExpr>, Box<SeppoExpr>),
    Assignment(String, Box<SeppoExpr>),
    Print(PrintFormat, Box<SeppoExpr>),
    Block(Vec<SeppoExpr>),
    Function(String, Vec<String>, Box<SeppoExpr>),
    FunctionCall(String, Vec<SeppoExpr>),
    Return(Box<SeppoExpr>),
    InlineC(String),
    Conditional {
        condition: Box<SeppoExpr>,
        true_block: Box<SeppoExpr>,
        false_block: Option<Box<SeppoExpr>>,
    },
}

#[derive(Debug, Clone)]
pub enum PrintFormat {
    Decimal,
    Hex,
}
