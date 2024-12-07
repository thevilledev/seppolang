use crate::types::*;
use anyhow::{anyhow, Result};
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "seppo.pest"]
pub struct SeppoParser;

pub fn parse_seppo(input: &str) -> Result<SeppoExpr> {
    println!("Input:\n{}", input);
    println!("Attempting to parse with Rule::program...");

    // Debug parsing
    println!("\nTrying program rule with detailed debugging:");
    let program_result = SeppoParser::parse(Rule::program, input);
    match &program_result {
        Ok(pairs) => {
            for pair in pairs.clone() {
                println!("\nTop level pair:");
                println!("Rule: {:?}", pair.as_rule());
                println!("Text: {}", pair.as_str());
                println!("Span: {:?}", pair.as_span());

                for inner in pair.into_inner() {
                    println!("\n  Inner pair:");
                    println!("  Rule: {:?}", inner.as_rule());
                    println!("  Text: {}", inner.as_str());
                    println!("  Span: {:?}", inner.as_span());
                }
            }
        }
        Err(e) => {
            println!("\nProgram parse error:");
            println!("Error: {:?}", e);
            println!("Location: {:?}", e.location);
            println!("Line/Col: {:?}", e.line_col);
        }
    }

    let pairs = program_result?;

    let mut functions = Vec::new();
    let mut has_main = false;

    for pair in pairs {
        match pair.as_rule() {
            Rule::program => {
                for item in pair.into_inner() {
                    match item.as_rule() {
                        Rule::function => {
                            let func_expr = parse_function(item)?;
                            if let SeppoExpr::Function(name, ..) = &func_expr {
                                if name == "seppo" {
                                    has_main = true;
                                }
                            }
                            functions.push(func_expr);
                        }
                        Rule::extern_block => {
                            let c_code = item
                                .into_inner()
                                .find(|p| p.as_rule() == Rule::c_code)
                                .map(|p| p.as_str().trim().to_string())
                                .ok_or_else(|| anyhow!("Expected C code in ceppo block"))?;
                            functions.push(SeppoExpr::InlineC(c_code));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    if !has_main {
        return Err(anyhow!("No seppo function found"));
    }

    Ok(SeppoExpr::Block(functions))
}

fn parse_function(pair: pest::iterators::Pair<Rule>) -> Result<SeppoExpr> {
    println!("Function rule: {:?}", pair.as_rule());
    assert_eq!(pair.as_rule(), Rule::function);
    for p in pair.clone().into_inner() {
        println!("  Child: {:?} = {:?}", p.as_rule(), p.as_str());
    }

    let mut inner = pair.into_inner();

    // Get function name
    let name = inner
        .next()
        .ok_or_else(|| anyhow!("Expected function name"))?
        .as_str()
        .to_string();

    // Parse parameters (empty for now since we don't have any in the input)
    let params = Vec::new();

    // Parse function body (block)
    let body = inner
        .next()
        .filter(|p| p.as_rule() == Rule::block)
        .ok_or_else(|| anyhow!("Expected function body"))?;

    println!("Body rule: {:?}", body.as_rule());

    Ok(SeppoExpr::Function(
        name,
        params,
        Box::new(parse_block(body)?),
    ))
}

fn parse_block(pair: pest::iterators::Pair<Rule>) -> Result<SeppoExpr> {
    let mut statements = Vec::new();
    for stmt in pair.into_inner() {
        statements.push(parse_statement(stmt)?);
    }
    Ok(SeppoExpr::Block(statements))
}

fn parse_statement(pair: pest::iterators::Pair<Rule>) -> Result<SeppoExpr> {
    match pair.as_rule() {
        Rule::statement => {
            let inner = pair.into_inner().next().unwrap();
            parse_statement(inner)
        }
        Rule::conditional_block => parse_conditional_block(pair),
        Rule::print_stmt => parse_print(pair),
        Rule::assignment => parse_assignment(pair),
        Rule::expression => parse_expression(pair),
        Rule::return_stmt => {
            println!("Parsing return: {:?}", pair.as_str()); // Debug
            let inner = pair
                .into_inner()
                .next()
                .ok_or_else(|| anyhow!("Expected return value"))?;
            Ok(SeppoExpr::Return(Box::new(parse_expression(inner)?)))
        }
        Rule::function_call => parse_function(pair),
        _ => Err(anyhow!(
            "Unexpected rule in statement: {:?}",
            pair.as_rule()
        )),
    }
}

fn parse_conditional_block(pair: pest::iterators::Pair<Rule>) -> Result<SeppoExpr> {
    let mut inner = pair.into_inner();

    // Parse condition
    let condition = inner.next().ok_or_else(|| anyhow!("Expected condition"))?;
    let condition_expr = parse_condition(condition)?;

    // Parse true block
    let true_block = inner.next().ok_or_else(|| anyhow!("Expected true block"))?;
    let true_expr = parse_block(true_block)?;

    // Parse optional false block (perkele block)
    let false_expr = inner.next().map(parse_block).transpose()?;

    Ok(SeppoExpr::Conditional {
        condition: Box::new(condition_expr),
        true_block: Box::new(true_expr),
        false_block: false_expr.map(Box::new),
    })
}

fn parse_condition(pair: pest::iterators::Pair<Rule>) -> Result<SeppoExpr> {
    let mut inner = pair.into_inner();
    let left = parse_expression(inner.next().unwrap())?;
    let op = inner.next().unwrap().as_str().to_string();
    let right = parse_expression(inner.next().unwrap())?;

    Ok(SeppoExpr::Operation(op, Box::new(left), Box::new(right)))
}

fn parse_print(pair: pest::iterators::Pair<Rule>) -> Result<SeppoExpr> {
    let mut inner = pair.into_inner();

    // Get the print command (seppo or 0xseppo)
    let command = inner
        .next()
        .ok_or_else(|| anyhow!("Expected print command"))?;
    let format = match command.as_str() {
        "0xseppo" => PrintFormat::Hex,
        _ => PrintFormat::Decimal,
    };

    // Get the expression to print
    let expr = inner
        .next()
        .ok_or_else(|| anyhow!("Expected expression to print"))?
        .into_inner()
        .next()
        .ok_or_else(|| anyhow!("Empty print expression"))?;

    let expr = parse_expression(expr)?;
    Ok(SeppoExpr::Print(format, Box::new(expr)))
}

fn parse_assignment(pair: pest::iterators::Pair<Rule>) -> Result<SeppoExpr> {
    let mut inner = pair.into_inner();
    let variable = inner.next().unwrap().as_str().to_string();
    let value_expr = parse_expression(inner.next().unwrap())?;
    Ok(SeppoExpr::Assignment(variable, Box::new(value_expr)))
}

fn parse_expression(pair: pest::iterators::Pair<Rule>) -> Result<SeppoExpr> {
    match pair.as_rule() {
        Rule::number => Ok(SeppoExpr::Number(pair.as_str().parse()?)),
        Rule::string_literal => {
            // Remove the quotes and handle escapes
            let str_content = pair.as_str();
            let str_without_quotes = &str_content[1..str_content.len() - 1];
            Ok(SeppoExpr::String(str_without_quotes.to_string()))
        }
        Rule::variable => Ok(SeppoExpr::Variable(pair.as_str().to_string())),
        Rule::identifier => Ok(SeppoExpr::Variable(pair.as_str().to_string())),
        Rule::operation => {
            let mut inner = pair.into_inner();
            let left = parse_expression(inner.next().unwrap())?;
            let op = inner.next().unwrap().as_str();
            let right = parse_expression(inner.next().unwrap())?;
            Ok(SeppoExpr::Operation(
                op.to_string(),
                Box::new(left),
                Box::new(right),
            ))
        }
        Rule::expression => {
            let inner = pair.into_inner().next().unwrap();
            parse_expression(inner)
        }
        Rule::function_call => {
            let mut inner = pair.into_inner();
            let name = inner.next().unwrap().as_str().to_string();
            let args = if let Some(arg_list) = inner.next() {
                arg_list
                    .into_inner()
                    .map(|arg| parse_expression(arg))
                    .collect::<Result<Vec<_>>>()?
            } else {
                Vec::new()
            };
            Ok(SeppoExpr::FunctionCall(name, args))
        }
        rule => Err(anyhow!("Unexpected rule in expression: {:?}", rule)),
    }
}
