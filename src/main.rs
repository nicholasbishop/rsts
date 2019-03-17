#[macro_use]
extern crate clap;

use std::fs;

#[derive(Debug)]
struct SimpleType {
    path: Vec<String>,
    // Generic args are only allowed in the final segment
    generic_args: Vec<SimpleType>,
}

#[derive(Debug)]
enum SimpleTypeError {
    QSelf,
    LeadingColon,
    EarlyGenericArgs,
    InvalidGenericArgType,
    InvalidArgType,
    TypeIsNotPath,
}

#[derive(Debug)]
struct SimpleField {
    name: Option<String>,
    ty: SimpleType,
}

impl SimpleField {
    fn new(name: Option<String>, ty: SimpleType) -> SimpleField {
        SimpleField { name, ty }
    }
}

#[derive(Debug)]
struct SimpleStruct {
    name: String,
    fields: Vec<SimpleField>,
}

const NUMERIC_TYPES: [&'static str; 10] = [
    "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64",
];

impl SimpleType {
    fn new(path: Vec<String>, generic_args: Vec<SimpleType>) -> SimpleType {
        SimpleType { path, generic_args }
    }

    fn from_syn_type(ty: &syn::Type) -> Result<SimpleType, SimpleTypeError> {
        if let syn::Type::Path(path) = ty {
            if path.qself.is_some() {
                return Err(SimpleTypeError::QSelf);
            }
            if path.path.leading_colon.is_some() {
                return Err(SimpleTypeError::LeadingColon);
            }

            let mut st = SimpleType::new(Vec::new(), Vec::new());
            for (i, seg) in path.path.segments.iter().enumerate() {
                let is_last = i == path.path.segments.len() - 1;
                if !is_last && !seg.arguments.is_empty() {
                    // Only allow generic arguments in the final
                    // segment
                    return Err(SimpleTypeError::EarlyGenericArgs);
                }
                st.path.push(seg.ident.to_string());

                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                    for arg in args.args.iter() {
                        if let syn::GenericArgument::Type(ty) = arg {
                            match SimpleType::from_syn_type(&ty) {
                                Ok(arg) => {
                                    st.generic_args.push(arg);
                                }
                                Err(err) => {
                                    return Err(err);
                                }
                            }
                        } else {
                            return Err(SimpleTypeError::InvalidGenericArgType);
                        }
                    }
                } else if !seg.arguments.is_empty() {
                    return Err(SimpleTypeError::InvalidArgType);
                }
            }

            Ok(st)
        } else {
            Err(SimpleTypeError::TypeIsNotPath)
        }
    }

    fn to_ts(&self) -> String {
        if self.path == ["Option"] && self.generic_args.len() == 1 {
            format!("{} | null", self.generic_args[0].to_ts())
        } else if self.path == ["Vec"] && self.generic_args.len() == 1 {
            let mut inner = self.generic_args[0].to_ts();
            if inner.contains(' ') {
                inner = format!("({})", inner);
            }
            format!("{}[]", inner)
        } else if self.generic_args.len() == 0 {
            if self.path.len() == 1 {
                if NUMERIC_TYPES.contains(&self.path[0].as_str()) {
                    "number".to_string()
                } else if self.path[0] == "String" {
                    "string".to_string()
                } else {
                    self.path[0].to_string()
                }
            } else {
                "TODO".to_string()
            }
        } else {
            "TODO".to_string()
        }
    }
}

impl SimpleStruct {
    fn new(s: &syn::ItemStruct) -> Option<SimpleStruct> {
        let name = s.ident.to_string();
        //        println!("{}", name);
        let mut ss = SimpleStruct {
            name,
            fields: Vec::new(),
        };
        for field in s.fields.iter() {
            let name = field.ident.as_ref().map(|i| i.to_string());
            match SimpleType::from_syn_type(&field.ty) {
                Ok(st) => {
                    ss.fields.push(SimpleField::new(name, st));
                    //println!("{}: {:?}", name, st);
                }
                Err(err) => {
                    println!("{:?}: {:?}", name, err);
                }
            }
            //println!("{}", field.ident.unwrap().to_string());
            //println!("{:#?}", field.ty);
            //println!("{:#?}", field.ty.to_tokens(&tokens));
        }
        Some(ss)
    }

    fn to_ts(&self) -> String {
        if self.fields.len() == 0 {
            panic!("empty structs not supported");
        } else if self.fields.len() == 1 && self.fields[0].name.is_none() {
            format!("type {} = {};", self.name, self.fields[0].ty.to_ts())
        } else {
            let mut out = format!("interface {} {{\n", self.name);
            for f in self.fields.iter() {
                out += &format!("  {}: {};\n", f.name.as_ref().unwrap(), f.ty.to_ts());
            }
            out += "}\n";
            out
        }
    }
}

struct SimpleFile {
    name: String,
    structs: Vec<SimpleStruct>,
}

impl SimpleFile {
    fn load(path: &std::path::Path) -> SimpleFile {
        let src = fs::read_to_string(path).expect("Unable to read file");

        let syntax = syn::parse_file(&src).expect("Unable to parse file");

        let mut structs = Vec::new();

        for item in syntax.items {
            if let syn::Item::Struct(s) = item {
                if let Some(s) = SimpleStruct::new(&s) {
                    structs.push(s);
                }
            }
        }

        SimpleFile {
            name: path.file_name().unwrap().to_str().unwrap().to_string(),
            structs: structs,
        }
    }

    fn to_ts(&self) -> String {
        let mut output = format!("// {}\n", self.name);
        for s in self.structs.iter() {
            output += &s.to_ts();
        }
        output
    }
}

fn main() {
    let matches = clap_app!(rsts =>
        (about: "Convert Rust types to Typescript")
        (@arg INPUT: +required +multiple "typescript file(s)")
    )
    .get_matches();

    //let inputs = matches.values_of("INPUT").unwrap().collect::<Vec<&str>>();

    let mut files = Vec::new();
    for input in matches.values_of("INPUT").unwrap() {
        files.push(SimpleFile::load(std::path::Path::new(input)));
    }

    //dbg!(&inputs);

    for f in files {
        print!("{}", f.to_ts());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_type_number() {
        let st = SimpleType::new(vec!["i32".to_string()], vec![]);
        assert_eq!(st.to_ts(), "number");
    }

    #[test]
    fn simple_type_string() {
        let st = SimpleType::new(vec!["String".to_string()], vec![]);
        assert_eq!(st.to_ts(), "string");
    }

    #[test]
    fn simple_type_option() {
        let st = SimpleType::new(
            vec!["Option".to_string()],
            vec![SimpleType {
                path: vec!["i32".to_string()],
                generic_args: vec![],
            }],
        );

        assert_eq!(st.to_ts(), "number | null");
    }

    #[test]
    fn simple_type_vec() {
        let st = SimpleType::new(
            vec!["Vec".to_string()],
            vec![SimpleType {
                path: vec!["i32".to_string()],
                generic_args: vec![],
            }],
        );

        assert_eq!(st.to_ts(), "number[]");
    }

    #[test]
    fn simple_type_vec_option() {
        let st = SimpleType::new(
            vec!["Vec".to_string()],
            vec![SimpleType::new(
                vec!["Option".to_string()],
                vec![SimpleType::new(vec!["i32".to_string()], vec![])],
            )],
        );

        assert_eq!(st.to_ts(), "(number | null)[]");
    }

    #[test]
    fn newtype() {
        let s = SimpleStruct {
            name: "MyType".to_string(),
            fields: vec![
                SimpleField::new(None, SimpleType::new(vec!["String".to_string()], vec![])),
            ]
        };

        assert_eq!(s.to_ts(), "type MyType = string;")
    }
}
