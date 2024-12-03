use crate::types::*;
use anyhow::{Result, anyhow};
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::builder::Builder;
use inkwell::values::{IntValue, PointerValue, FunctionValue};
use std::collections::HashMap;
use inkwell::targets::{
    CodeModel, FileType, RelocMode, Target, 
    TargetMachine
};
use std::path::Path;

pub struct CodeGen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    variables: HashMap<String, PointerValue<'ctx>>,
    functions: HashMap<String, FunctionValue<'ctx>>,
    current_function: Option<FunctionValue<'ctx>>,
}

impl<'ctx> CodeGen<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();

        // Add printf declaration
        let i32_type = context.i32_type();
        let printf_type = i32_type.fn_type(&[context.ptr_type(0.into()).into()], true);
        module.add_function("printf", printf_type, None);

        Self {
            context,
            module,
            builder,
            variables: HashMap::new(),
            functions: HashMap::new(),
            current_function: None,
        }
    }

    pub fn compile(&mut self, expr: &SeppoExpr) -> Result<()> {
        // Don't create a main function here anymore, just generate code for the expression
        self.gen_expr(expr)?;

        // Print LLVM IR for debugging
        println!("LLVM IR:\n{}", self.module.print_to_string().to_string());

        // Verify module
        if self.module.verify().is_err() {
            return Err(anyhow!("Module verification failed"));
        }
        
        Ok(())
    }

    fn gen_expr(&mut self, expr: &SeppoExpr) -> Result<IntValue<'ctx>> {
        match expr {
            SeppoExpr::Number(n) => {
                Ok(self.context.i64_type().const_int(*n as u64, false))
            },
            SeppoExpr::Variable(name) => {
                if let Some(ptr) = self.variables.get(name) {
                    let load = self.builder.build_load(self.context.i64_type(), *ptr, name)?;
                    Ok(load.into_int_value())
                } else {
                    Err(anyhow!("Undefined variable: {}", name))
                }
            },
            SeppoExpr::Function(name, params, body) => {
                let i64_type = self.context.i64_type();
                let param_types = vec![i64_type.into(); params.len()];
                let fn_type = i64_type.fn_type(&param_types, false);
                let function = self.module.add_function(name, fn_type, None);
                
                // Store function for later use
                self.functions.insert(name.clone(), function);
                
                // Create entry block
                let entry = self.context.append_basic_block(function, "entry");
                self.builder.position_at_end(entry);
                
                // Save current function
                let prev_function = self.current_function;
                self.current_function = Some(function);
                
                // Create new scope for variables
                let prev_vars = self.variables.clone();
                self.variables.clear();
                
                // Add parameters to variables
                for (i, param) in params.iter().enumerate() {
                    let alloca = self.builder.build_alloca(i64_type, param)?;
                    self.builder.build_store(alloca, function.get_nth_param(i as u32).unwrap())?;
                    self.variables.insert(param.clone(), alloca);
                }
                
                // Generate body
                let result = self.gen_expr(body)?;
                
                // Only build return if the last instruction wasn't already a return
                if !self.builder.get_insert_block().unwrap().get_terminator().is_some() {
                    self.builder.build_return(Some(&result))?;
                }
                
                // Restore previous scope
                self.variables = prev_vars;
                self.current_function = prev_function;
                
                Ok(i64_type.const_int(0, false))
            },
            SeppoExpr::FunctionCall(name, args) => {
                if let Some(&function) = self.functions.get(name) {
                    let compiled_args: Vec<_> = args.iter()
                        .map(|arg| self.gen_expr(arg))
                        .collect::<Result<Vec<_>>>()?
                        .into_iter()
                        .map(|val| val.into())
                        .collect();
                    
                    let result = self.builder.build_call(function, &compiled_args, "calltmp")?;
                    Ok(result.try_as_basic_value().left().unwrap().into_int_value())
                } else {
                    Err(anyhow!("Undefined function: {}", name))
                }
            },
            SeppoExpr::Return(value) => {
                let return_value = self.gen_expr(value)?;
                if let Some(_) = self.current_function {
                    self.builder.build_return(Some(&return_value))?;
                    // Return the value but don't generate more code after this
                    Ok(return_value)
                } else {
                    Err(anyhow!("Return statement outside of function"))
                }
            },
            SeppoExpr::Operation(op, left, right) => {
                let lhs = self.gen_expr(left)?;
                let rhs = self.gen_expr(right)?;
                
                match op.as_str() {
                    "+" => self.builder.build_int_add(lhs, rhs, "addtmp").map_err(|e| anyhow!(e)),
                    "-" => self.builder.build_int_sub(lhs, rhs, "subtmp").map_err(|e| anyhow!(e)),
                    "*" => self.builder.build_int_mul(lhs, rhs, "multmp").map_err(|e| anyhow!(e)),
                    "/" => self.builder.build_int_signed_div(lhs, rhs, "divtmp").map_err(|e| anyhow!(e)),
                    _ => Err(anyhow!("Unknown operator: {}", op)),
                }
            },
            SeppoExpr::Assignment(name, value) => {
                let val = self.gen_expr(value)?;
                
                let alloca = if let Some(ptr) = self.variables.get(name) {
                    *ptr
                } else {
                    let alloca = self.builder.build_alloca(self.context.i64_type(), name)?;
                    self.variables.insert(name.clone(), alloca);
                    alloca
                };
                
                self.builder.build_store(alloca, val)?;
                Ok(val)
            },
            SeppoExpr::Print(expr) => {
                let value = self.gen_expr(expr)?;
                
                let printf = self.module.get_function("printf").unwrap();
                let format_string = self.builder.build_global_string_ptr("%ld\n", "format_string")?;
                
                self.builder.build_call(
                    printf,
                    &[format_string.as_pointer_value().into(), value.into()],
                    "printf_call",
                )?;
                
                Ok(value)
            },
            SeppoExpr::Block(expressions) => {
                let mut last_value = self.context.i64_type().const_int(0, false);
                for expr in expressions {
                    last_value = self.gen_expr(expr)?;
                    // Don't generate code after a return instruction
                    if matches!(expr, SeppoExpr::Return(_)) {
                        break;
                    }
                }
                Ok(last_value)
            },
        }
    }

    pub fn get_module(&self) -> &Module<'ctx> {
        &self.module
    }

    pub fn write_object_file(&self, output: &Path) -> Result<()> {
        // Get host target triple
        let target_triple = TargetMachine::get_default_triple();
        let cpu = TargetMachine::get_host_cpu_name().to_string();
        let features = TargetMachine::get_host_cpu_features().to_string();
        
        // Initialize target
        let target = Target::from_triple(&target_triple)
            .map_err(|e| anyhow!("Failed to get target: {}", e))?;
            
        // Create target machine
        let target_machine = target
            .create_target_machine(
                &target_triple,
                &cpu,
                &features,
                inkwell::OptimizationLevel::Default,
                RelocMode::Default,
                CodeModel::Default,
            )
            .ok_or_else(|| anyhow!("Failed to create target machine"))?;

        // Write object file
        target_machine
            .write_to_file(
                &self.module,
                FileType::Object,
                output,
            )
            .map_err(|e| anyhow!("Failed to write object file: {}", e))
    }
} 