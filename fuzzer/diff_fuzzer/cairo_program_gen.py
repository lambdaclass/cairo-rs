"""
Generate a cairo program with the following rules:
    1. Grab a hint
    2. Look for all the ids.(...) expressions, make sure to keep track of any "=" to the left or right
        - if none, go back to 1
    3. Reduce the ids.(...) expressions so that all the variables + their fields are grouped
    4. Create inputs and outputs variables dicts
        - inputs: if "=" was to the right
        - outputs: if "=" was to the left
    5. Create main using input variables
        - func main() {
            let a = MyStruct(field=1, field=2);
            hint_func();
            return();
          }
    6. Create hint_func with outputs as locals
        - hint_func(a: MyStruct) -> (MyStruct) {
            alloc_locals;
            local b: MyStruct;
            %{
              ...
            %}
            return(b);
          }
"""

def multi_replace(in_str, patterns):
    """
    multi_replace(String, String) -> String
    Replace multiple characters for space
    """
    return "".join([ c if c not in patterns else " " for c in in_str ])

def var_in_pack(line, stripped_var):
    """
    var_in_pack(String, String) -> bool
    Check if a variable is inside a pack(variable, PRIME) call.
    This function expects a stripped variable, without the `ids.` prefix.
    Also, the current implemenation doesn't deal with parenthesis inside the `pack` call
    """
    if "pack(" not in line:
        return False
    var = "ids." + stripped_var
    # Assuming there is no inner parenthesis in pack(...)
    pack_start = line.find("pack(")
    pack_end = line.find(")", pack_start)
    return var in line[pack_start:pack_end]
     
    
def classify_variables(hint_code):
    """
    classify_varables(String) -> (Dict, Dict, Dict)
    Takes a hint code block and extract all the variables with the `ids.` prefix, classifying each one into
    dictionaries representing input, output and inout variables.
    Then if the variable has fields, annotate them in the corresponding dictionary, otherwise it's normally
    considered a felt execpt for special cases like being passed as an argument to a `pack(variable, PRIME)`
    function.
    """
    input_vars = dict()
    output_vars = dict()
    inout_vars = dict()
    imported_variables = []
    extra_hints = ""
    block_permutation_set = False
    lines = [multi_replace(line, '",)]}(') for line in hint_code.split("\n") if "ids." in line]
  
    for line in lines:
        
        variables = [v for v in line.split() if "ids." in v]

        for var in variables:
            var.replace(".", "", -1)
            dict_to_insert = dict()

            if line.find(var) < line.find("=") < line.find(var, line.find("=")):
                dict_to_insert = inout_vars
            elif line.find("=") < line.find(var):
                dict_to_insert = input_vars
            else:
                dict_to_insert = output_vars

            # Remove "ids."
            var_field = var.split(".")[1:]
            if var_in_pack(line, var_field[0]):
                # If the variable is inside a pack(...) function, make sure it's a point
                dict_to_insert[var_field[0]] = { "d0", "d1", "d2" }
            elif len(var_field) == 1:
                dict_to_insert[var_field[0]] = "felt"
            else:
                if not var_field[0] in dict_to_insert or dict_to_insert[var_field[0]] == "felt":
                    dict_to_insert[var_field[0]] = { var_field[1] }
                else:
                    dict_to_insert[var_field[0]].add(var_field[1])

    input_vars.update((k, tuple(v) if v != "felt" else "felt") for (k, v) in input_vars.items())
    output_vars.update((k, tuple(v) if v != "felt" else "felt") for (k, v) in output_vars.items())
    inout_vars.update((k, tuple(v) if v != "felt" else "felt") for (k, v) in inout_vars.items())

    input_vars = (input_vars | inout_vars)
    return input_vars, output_vars, inout_vars

def generate_cairo_hint_program(hint_code):
    """
    generate_cairo_hint_program(String) -> [String]
    Call `classify_varables(hint_code)` and create a cairo program with all the necessary code to run the hint
    code block that was passed as parameter
    """
    input_vars, output_vars, inout_vars = classify_variables(hint_code)

    fields = { v for v in (input_vars | output_vars).values() }
    structs_dict = { v : "MyStruct" + str(i) for (i, v) in enumerate(fields) if v != "felt"}
    structs_dict["felt"] = "felt"

    structs_fmt = "struct {struct_name} {{\n{struct_fields}\n}}"
    fields_fmt = "\t{field_name}: felt,"
    
    declared_structs = "\n".join([
        structs_fmt.format(
            struct_name = name,
            struct_fields = "\n".join([fields_fmt.format(field_name=field_name) for field_name in fields])
        )
        for (fields, name) in structs_dict.items() if name != "felt"
    ])


    main_func_fmt = "func main() {{{variables}\n\thint_func({input_var_names});\n\treturn();\n}}"
    main_struct_assignment_fmt = "\n\tlet {var_name} = {struct_name}({assign_fields});"
    main_var_felt_assingment_fmt = "\n\tlet {var_name} =;"

    main_var_assignments = ""
    for name, var_fields in input_vars.items():
        main_var_assignments += \
            main_var_felt_assingment_fmt.format(var_name = name) if structs_dict[var_fields] == "felt" else \
            main_struct_assignment_fmt.format(
                var_name = name,
                struct_name = structs_dict[var_fields],
                assign_fields = ", ".join([
                    field_name + "=" for field_name in var_fields
                ])
            )

    main_func = main_func_fmt.format(
        variables = main_var_assignments,
        input_var_names = ", ".join([var for var in input_vars.keys()])
    )

    hint_func_fmt = "func hint_func{signature} {{\n\talloc_locals;\n{local_declarations}\n%{{\n{hint}\n%}}\n\treturn({output_return});\n}}"

    hint_input_var_fmt = "{var_name}: {struct_name}"
    local_declare_fmt = "\tlocal {res_var_name}: {res_struct};"


    if len(output_vars | inout_vars) == 1:
        signature =  "(" + \
            ", ".join([
                hint_input_var_fmt.format(var_name = name, struct_name = structs_dict[var_fields]) for name, var_fields in input_vars.items()
            ]) + \
            ") -> " + \
            "".join([structs_dict[var_fields] for var_fields in (output_vars | inout_vars).values()]) 
            
    else: 
        signature = "(" + \
            ", ".join([
                hint_input_var_fmt.format(var_name = name, struct_name = structs_dict[var_fields]) for name, var_fields in input_vars.items()
            ]) + \
            ") -> (" + \
            ", ".join([structs_dict[var_fields] for var_fields in (output_vars | inout_vars).values()]) + \
            ")"
    
    local_vars = "\n".join([
        local_declare_fmt.format(res_var_name = name, res_struct = structs_dict[var_fields]) for name, var_fields in output_vars.items()
    ])

    hint_func = hint_func_fmt.format(
        signature = signature, 
        local_declarations = local_vars, 
        hint = hint_code,
        output_return = ", ".join([res for res in (output_vars | inout_vars).keys()])
    )

    return declared_structs.split("\n") + main_func.split("\n") + hint_func.split("\n")
