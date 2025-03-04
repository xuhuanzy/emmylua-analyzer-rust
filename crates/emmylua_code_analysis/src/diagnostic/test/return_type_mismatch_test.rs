#[cfg(test)]
mod tests {
    use crate::{DiagnosticCode, VirtualWorkspace};

    #[test]
    fn test_return_type_mismatch() {
        let mut ws = VirtualWorkspace::new();

        assert!(!ws.check_code_for(
            DiagnosticCode::ReturnTypeMismatch,
            r#"
            ---@class diagnostic.Test1
            local Test = {}

            ---@return number
            function Test.n()
                return "1"
            end
        "#
        ));

        assert!(!ws.check_code_for(
            DiagnosticCode::ReturnTypeMismatch,
            r#"
            ---@return string
            local test = function()
                return 1
            end
        "#
        ));

        assert!(!ws.check_code_for(
            DiagnosticCode::ReturnTypeMismatch,
            r#"
            ---@class diagnostic.Test2
            local Test = {}

            ---@return number
            Test.n = function ()
                return "1"
            end
        "#
        ));
        assert!(!ws.check_code_for(
            DiagnosticCode::ReturnTypeMismatch,
            r#"
            ---@return number
            local function test3()
                if true then
                    return ""
                else
                    return 2, 3
                end
                return 2, 3
            end
        "#
        ));
    }

    #[test]
    fn test_dots() {
        let mut ws = VirtualWorkspace::new();

        assert!(ws.check_code_for(
            DiagnosticCode::RedundantReturnValue,
            r#"
            ---@return number, ...
            local function test()
                return 1, 2, 3
            end
        "#  
        ));
        assert!(ws.check_code_for(
            DiagnosticCode::RedundantReturnValue,
            r#"
            ---@return string ret2
            ---@return ... ret
            local function test3()
                return 1, 2, "3"
            end
        "#  
        ));
    }


    #[test]
    fn test_missing_return_value() {
        let mut ws = VirtualWorkspace::new();

        assert!(!ws.check_code_for(
            DiagnosticCode::MissingReturnValue,
            r#"
            ---@return number
            local function test()
                return
            end
        "#
        ));
    }

    #[test]
    fn test_redundant_return_value() {
        let mut ws = VirtualWorkspace::new();

        assert!(!ws.check_code_for(
            DiagnosticCode::RedundantReturnValue,
            r#"
            ---@return number
            local function test()
                return 1, 2
            end
        "#
        ));
    }
}
