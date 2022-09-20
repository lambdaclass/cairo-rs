use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use num_bigint::BigInt;

use crate::any_box;
use crate::hint_processor::builtin_hint_processor::dict_manager::DictManager;
use crate::types::exec_scope::ExecutionScopes;
use crate::vm::errors::exec_scope_errors::ExecScopeError;
use crate::vm::errors::vm_errors::VirtualMachineError;

///Structure representing a limited access to the execution scopes
///Allows adding and removing scopes, but will only allow modifications to the last scope present before hint execution
pub struct ExecutionScopesProxy<'a> {
    scopes: &'a mut ExecutionScopes,
    current_scope: usize,
}

///Creates an ExecutionScopeProxy from the execution_scopes
pub fn get_exec_scopes_proxy(exec_scopes: &mut ExecutionScopes) -> ExecutionScopesProxy {
    ExecutionScopesProxy {
        //Len will always be > 1 as execution scopes are always created with a main scope
        current_scope: exec_scopes.data.len() - 1,
        scopes: exec_scopes,
    }
}

impl ExecutionScopesProxy<'_> {
    ///Creates a new scope with predefined variables
    pub fn enter_scope(&mut self, new_scope_locals: HashMap<String, Box<dyn Any>>) {
        self.scopes.enter_scope(new_scope_locals);
    }

    ///Removes last scope from exec_scopes
    pub fn exit_scope(&mut self) -> Result<(), ExecScopeError> {
        self.scopes.exit_scope()
    }

    ///Creates or updates an existing variable given its name and boxed value
    pub fn assign_or_update_variable(&mut self, var_name: &str, var_value: Box<dyn Any>) {
        if let Ok(local_variables) = self.get_local_variables_mut() {
            local_variables.insert(var_name.to_string(), var_value);
        }
    }

    ///Removes a variable from the current scope given its name
    pub fn delete_variable(&mut self, var_name: &str) {
        if let Ok(local_variables) = self.get_local_variables_mut() {
            local_variables.remove(var_name);
        }
    }

    ///Returns a mutable reference to the dictionary containing the variables present in the current scope
    pub fn get_local_variables_mut(
        &mut self,
    ) -> Result<&mut HashMap<String, Box<dyn Any>>, VirtualMachineError> {
        if self.scopes.data.len() > self.current_scope {
            return Ok(&mut self.scopes.data[self.current_scope]);
        }
        Err(VirtualMachineError::MainScopeError(
            ExecScopeError::NoScopeError,
        ))
    }

    ///Returns a dictionary containing the variables present in the current scope
    pub fn get_local_variables(
        &self,
    ) -> Result<&HashMap<String, Box<dyn Any>>, VirtualMachineError> {
        if self.scopes.data.len() > self.current_scope {
            return Ok(&self.scopes.data[self.current_scope]);
        }
        Err(VirtualMachineError::MainScopeError(
            ExecScopeError::NoScopeError,
        ))
    }

    ///Returns the value in the current execution scope that matches the name and is of type BigInt
    pub fn get_int(&self, name: &str) -> Result<BigInt, VirtualMachineError> {
        let mut val: Option<BigInt> = None;
        if let Some(variable) = self.get_local_variables()?.get(name) {
            if let Some(int) = variable.downcast_ref::<BigInt>() {
                val = Some(int.clone());
            }
        }
        val.ok_or_else(|| VirtualMachineError::VariableNotInScopeError(name.to_string()))
    }

    ///Returns the value in the current execution scope that matches the name
    pub fn get_any_boxed_ref(&self, name: &str) -> Result<&Box<dyn Any>, VirtualMachineError> {
        if let Some(variable) = self.get_local_variables()?.get(name) {
            return Ok(variable);
        }
        Err(VirtualMachineError::VariableNotInScopeError(
            name.to_string(),
        ))
    }

    ///Returns the value in the current execution scope that matches the name
    pub fn get_any_boxed_mut(
        &mut self,
        name: &str,
    ) -> Result<&mut Box<dyn Any>, VirtualMachineError> {
        if let Some(variable) = self.get_local_variables_mut()?.get_mut(name) {
            return Ok(variable);
        }
        Err(VirtualMachineError::VariableNotInScopeError(
            name.to_string(),
        ))
    }

    ///Returns a reference to the value in the current execution scope that matches the name and is of type BigInt
    pub fn get_int_ref(&self, name: &str) -> Result<&BigInt, VirtualMachineError> {
        let mut val: Option<&BigInt> = None;
        if let Some(variable) = self.get_local_variables()?.get(name) {
            if let Some(int) = variable.downcast_ref::<BigInt>() {
                val = Some(int);
            }
        }
        val.ok_or_else(|| VirtualMachineError::VariableNotInScopeError(name.to_string()))
    }

    ///Returns a mutable reference to the value in the current execution scope that matches the name and is of type BigInt
    pub fn get_mut_int_ref(&mut self, name: &str) -> Result<&mut BigInt, VirtualMachineError> {
        let mut val: Option<&mut BigInt> = None;
        if let Some(variable) = self.get_local_variables_mut()?.get_mut(name) {
            if let Some(int) = variable.downcast_mut::<BigInt>() {
                val = Some(int);
            }
        }
        val.ok_or_else(|| VirtualMachineError::VariableNotInScopeError(name.to_string()))
    }

    ///Returns the value in the current execution scope that matches the name and is of type List
    pub fn get_list(&self, name: &str) -> Result<Vec<BigInt>, VirtualMachineError> {
        let mut val: Option<Vec<BigInt>> = None;
        if let Some(variable) = self.get_local_variables()?.get(name) {
            if let Some(list) = variable.downcast_ref::<Vec<BigInt>>() {
                val = Some(list.clone());
            }
        }
        val.ok_or_else(|| VirtualMachineError::VariableNotInScopeError(name.to_string()))
    }

    ///Returns a reference to the value in the current execution scope that matches the name and is of type List
    pub fn get_list_ref(&self, name: &str) -> Result<&Vec<BigInt>, VirtualMachineError> {
        let mut val: Option<&Vec<BigInt>> = None;
        if let Some(variable) = self.get_local_variables()?.get(name) {
            if let Some(list) = variable.downcast_ref::<Vec<BigInt>>() {
                val = Some(list);
            }
        }
        val.ok_or_else(|| VirtualMachineError::VariableNotInScopeError(name.to_string()))
    }

    ///Returns a mutable reference to the value in the current execution scope that matches the name and is of type List
    pub fn get_mut_list_ref(
        &mut self,
        name: &str,
    ) -> Result<&mut Vec<BigInt>, VirtualMachineError> {
        let mut val: Option<&mut Vec<BigInt>> = None;
        if let Some(variable) = self.get_local_variables_mut()?.get_mut(name) {
            if let Some(list) = variable.downcast_mut::<Vec<BigInt>>() {
                val = Some(list);
            }
        }
        val.ok_or_else(|| VirtualMachineError::VariableNotInScopeError(name.to_string()))
    }

    ///Returns the value in the current execution scope that matches the name and is of type ListU64
    pub fn get_listu64(&self, name: &str) -> Result<Vec<u64>, VirtualMachineError> {
        let mut val: Option<Vec<u64>> = None;
        if let Some(variable) = self.get_local_variables()?.get(name) {
            if let Some(list) = variable.downcast_ref::<Vec<u64>>() {
                val = Some(list.clone());
            }
        }
        val.ok_or_else(|| VirtualMachineError::VariableNotInScopeError(name.to_string()))
    }
    ///Returns a reference to the value in the current execution scope that matches the name and is of type ListU64
    pub fn get_listu64_ref(&self, name: &str) -> Result<&Vec<u64>, VirtualMachineError> {
        let mut val: Option<&Vec<u64>> = None;
        if let Some(variable) = self.get_local_variables()?.get(name) {
            if let Some(list) = variable.downcast_ref::<Vec<u64>>() {
                val = Some(list);
            }
        }
        val.ok_or_else(|| VirtualMachineError::VariableNotInScopeError(name.to_string()))
    }
    ///Returns a mutable reference to the value in the current execution scope that matches the name and is of type ListU64
    pub fn get_mut_listu64_ref(
        &mut self,
        name: &str,
    ) -> Result<&mut Vec<u64>, VirtualMachineError> {
        let mut val: Option<&mut Vec<u64>> = None;
        if let Some(variable) = self.get_local_variables_mut()?.get_mut(name) {
            if let Some(list) = variable.downcast_mut::<Vec<u64>>() {
                val = Some(list);
            }
        }
        val.ok_or_else(|| VirtualMachineError::VariableNotInScopeError(name.to_string()))
    }

    ///Returns the value in the current execution scope that matches the name and is of type ListU64
    pub fn get_u64(&self, name: &str) -> Result<u64, VirtualMachineError> {
        let mut val: Option<u64> = None;
        if let Some(variable) = self.get_local_variables()?.get(name) {
            if let Some(num) = variable.downcast_ref::<u64>() {
                val = Some(*num);
            }
        }
        val.ok_or_else(|| VirtualMachineError::VariableNotInScopeError(name.to_string()))
    }
    ///Returns a reference to the value in the current execution scope that matches the name and is of type U64
    pub fn get_u64_ref(&self, name: &str) -> Result<&u64, VirtualMachineError> {
        let mut val: Option<&u64> = None;
        if let Some(variable) = self.get_local_variables()?.get(name) {
            if let Some(num) = variable.downcast_ref::<u64>() {
                val = Some(num);
            }
        }
        val.ok_or_else(|| VirtualMachineError::VariableNotInScopeError(name.to_string()))
    }
    ///Returns a mutable reference to the value in the current execution scope that matches the name and is of type U64
    pub fn get_mut_u64_ref(&mut self, name: &str) -> Result<&mut u64, VirtualMachineError> {
        let mut val: Option<&mut u64> = None;
        if let Some(variable) = self.get_local_variables_mut()?.get_mut(name) {
            if let Some(num) = variable.downcast_mut::<u64>() {
                val = Some(num);
            }
        }
        val.ok_or_else(|| VirtualMachineError::VariableNotInScopeError(name.to_string()))
    }

    ///Returns the value in the dict manager
    pub fn get_dict_manager(&self) -> Result<Rc<RefCell<DictManager>>, VirtualMachineError> {
        let mut val: Option<Rc<RefCell<DictManager>>> = None;
        if let Some(variable) = self.get_local_variables()?.get("dict_manager") {
            if let Some(dict_manager) = variable.downcast_ref::<Rc<RefCell<DictManager>>>() {
                val = Some(dict_manager.clone());
            }
        }
        val.ok_or_else(|| VirtualMachineError::VariableNotInScopeError("dict_manager".to_string()))
    }

    ///Returns a mutable reference to the value in the current execution scope that matches the name and is of type DictBigIntListU64
    pub fn get_mut_dict_int_list_u64_ref(
        &mut self,
        name: &str,
    ) -> Result<&mut HashMap<BigInt, Vec<u64>>, VirtualMachineError> {
        let mut val: Option<&mut HashMap<BigInt, Vec<u64>>> = None;
        if let Some(variable) = self.get_local_variables_mut()?.get_mut(name) {
            if let Some(dict) = variable.downcast_mut::<HashMap<BigInt, Vec<u64>>>() {
                val = Some(dict);
            }
        }
        val.ok_or_else(|| VirtualMachineError::VariableNotInScopeError(name.to_string()))
    }

    ///Inserts the boxed value into the current scope
    pub fn insert_box(&mut self, name: &str, value: Box<dyn Any>) {
        self.assign_or_update_variable(name, value);
    }
    ///Inserts the value into the current scope
    pub fn insert_value<T: 'static>(&mut self, name: &str, value: T) {
        self.assign_or_update_variable(name, any_box!(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bigint;

    #[test]
    fn get_local_variables_mut_main_scope_error() {
        let mut exec_scopes_proxy = ExecutionScopesProxy {
            scopes: &mut ExecutionScopes { data: Vec::new() },
            current_scope: 5,
        };

        let error = exec_scopes_proxy.get_local_variables_mut();
        assert!(error.is_err());
        assert_eq!(
            error.unwrap_err(),
            VirtualMachineError::MainScopeError(ExecScopeError::NoScopeError)
        );
    }

    #[test]
    fn get_local_variables_main_scope_error() {
        let exec_scopes_proxy = ExecutionScopesProxy {
            scopes: &mut ExecutionScopes { data: Vec::new() },
            current_scope: 5,
        };

        let error = exec_scopes_proxy.get_local_variables();
        assert!(error.is_err());
        assert_eq!(
            error.unwrap_err(),
            VirtualMachineError::MainScopeError(ExecScopeError::NoScopeError)
        );
    }

    #[test]
    fn get_any_boxed_ref_var_not_in_scope() {
        let exec_scopes_proxy = ExecutionScopesProxy {
            scopes: &mut ExecutionScopes {
                data: vec![HashMap::from([("a".to_string(), any_box!(1))])],
            },
            current_scope: 0,
        };

        let error = exec_scopes_proxy.get_any_boxed_ref("num");
        assert!(error.is_err());
        assert_eq!(
            error.unwrap_err(),
            VirtualMachineError::VariableNotInScopeError("num".to_string())
        );
    }

    #[test]
    fn get_any_boxed_mut_var_ok() {
        let exec_scopes_proxy = ExecutionScopesProxy {
            scopes: &mut ExecutionScopes {
                data: vec![HashMap::from([("num".to_string(), any_box!(1))])],
            },
            current_scope: 0,
        };

        assert!(exec_scopes_proxy.get_any_boxed_ref("num").is_ok());
    }

    #[test]
    fn get_any_boxed_mut_var_not_in_scope() {
        let exec_scopes_proxy = ExecutionScopesProxy {
            scopes: &mut ExecutionScopes {
                data: vec![HashMap::from([("a".to_string(), any_box!(1))])],
            },
            current_scope: 0,
        };

        let error = exec_scopes_proxy.get_any_boxed_ref("num");
        assert!(error.is_err());
        assert_eq!(
            error.unwrap_err(),
            VirtualMachineError::VariableNotInScopeError("num".to_string())
        );
    }

    #[test]
    fn get_mut_int_ref_sucess() {
        let mut exec_scopes_proxy = ExecutionScopesProxy {
            scopes: &mut ExecutionScopes {
                data: vec![HashMap::from([("a".to_string(), any_box!(bigint!(1)))])],
            },
            current_scope: 0,
        };

        assert_eq!(exec_scopes_proxy.get_mut_int_ref("a"), Ok(&mut bigint!(1)));
    }

    #[test]
    fn get_list_u64_sucess() {
        let exec_scopes_proxy = ExecutionScopesProxy {
            scopes: &mut ExecutionScopes {
                data: vec![HashMap::from([("a".to_string(), any_box!(vec![2_u64]))])],
            },
            current_scope: 0,
        };

        assert_eq!(exec_scopes_proxy.get_listu64("a"), Ok(vec![2_u64]));
    }

    #[test]
    fn get_u64_ref_sucess() {
        let exec_scopes_proxy = ExecutionScopesProxy {
            scopes: &mut ExecutionScopes {
                data: vec![HashMap::from([("a".to_string(), any_box!(3_u64))])],
            },
            current_scope: 0,
        };

        assert_eq!(exec_scopes_proxy.get_u64_ref("a"), Ok(&3_u64));
    }

    #[test]
    fn get_mut_u64_ref_sucess() {
        let mut exec_scopes_proxy = ExecutionScopesProxy {
            scopes: &mut ExecutionScopes {
                data: vec![HashMap::from([("a".to_string(), any_box!(4_u64))])],
            },
            current_scope: 0,
        };

        assert_eq!(exec_scopes_proxy.get_mut_u64_ref("a"), Ok(&mut 4_u64));
    }

    #[test]
    fn insert_box_sucess() {
        let mut exec_scopes_proxy = ExecutionScopesProxy {
            scopes: &mut ExecutionScopes {
                data: vec![HashMap::from([("a".to_string(), any_box!(3_u64))])],
            },
            current_scope: 0,
        };

        exec_scopes_proxy.insert_box("b", any_box!(7_u64));
        assert_eq!(exec_scopes_proxy.get_u64_ref("b"), Ok(&7_u64));
    }
}
