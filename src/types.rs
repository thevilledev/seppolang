#[derive(Debug, Clone, PartialEq)]
pub enum SeppoExpr {
    Number(i64),
    Variable(String),
    Operation(String, Box<SeppoExpr>, Box<SeppoExpr>),
    Assignment(String, Box<SeppoExpr>),
    Print(Box<SeppoExpr>),
    Block(Vec<SeppoExpr>),
    Function(String, Vec<String>, Box<SeppoExpr>),
    FunctionCall(String, Vec<SeppoExpr>),
    Return(Box<SeppoExpr>),
}