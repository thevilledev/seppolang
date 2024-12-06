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
    
    // Try parsing with program rule and print each step
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
    
    // Original parse
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
                                if name == "main" {
                                    has_main = true;
                                }
                            }
                            functions.push(func_expr);
                        }
                        Rule::extern_block => {
                            let c_code = item
                                .into_inner()
                                .find(|p| p.as_rule() == Rule::c_content)
                                .map(|p| p.as_str().to_string())
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
        return Err(anyhow!("No main function found"));
    }

    Ok(SeppoExpr::Block(functions))
}

fn parse_function(pair: pest::iterators::Pair<Rule>) -> Result<SeppoExpr> {
    println!("Function rule: {:?}", pair.as_rule());
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
        Rule::function => parse_function(pair),
        _ => Err(anyhow!(
            "Unexpected rule in statement: {:?}",
            pair.as_rule()
        )),
    }
}

fn parse_print(pair: pest::iterators::Pair<Rule>) -> Result<SeppoExpr> {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::print_item => {
            let expr_pair = inner.into_inner().next().unwrap();
            let expr = parse_expression(expr_pair)?;
            Ok(SeppoExpr::Print(Box::new(expr)))
        }
        _ => Err(anyhow!("Unexpected rule in print: {:?}", inner.as_rule())),
    }
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
