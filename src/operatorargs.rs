use std::collections::HashMap;
use yaml_rust::{Yaml, YamlEmitter, YamlLoader};

#[derive(Debug, Clone, Default)]
pub struct OperatorArgs {
    pub name: String,
    pub args: HashMap<String, String>,
    pub used: HashMap<String, String>,
    pub all_used: HashMap<String, String>,
}

impl OperatorArgs {
    #[must_use]
    pub fn new() -> OperatorArgs {
        OperatorArgs {
            name: String::new(),
            args: HashMap::new(),
            used: HashMap::new(),
            all_used: HashMap::new(),
        }
    }

    /// Provides an `OperatorArgs` object, populated by the global defaults (`ellps: GRS80`)
    #[must_use]
    pub fn global_defaults() -> OperatorArgs {
        let mut op = OperatorArgs {
            name: String::new(),
            args: HashMap::new(),
            used: HashMap::new(),
            all_used: HashMap::new(),
        };
        op.insert("ellps", "GRS80");
        op
    }

    /// Provides an `OperatorArgs` object, populated by the defaults from an existing
    /// `OperatorArgs`, combined with a new object definition.
    ///
    /// This is the mechanism for inheritance of global args in pipelines.
    #[must_use]
    pub fn with_globals_from(
        existing: &OperatorArgs,
        definition: &str,
        which: &str,
    ) -> OperatorArgs {
        let mut oa = OperatorArgs::new();
        for (arg, val) in &existing.args {
            if arg.starts_with('_') || (arg == "inv") {
                continue;
            }
            oa.insert(arg, val);
        }
        oa.populate(definition, which);
        oa
    }

    ///
    /// Insert PROJ style operator definition arguments, converted from a YAML
    /// setup string.
    ///
    /// If `which` is set to the empty string, we first look for a pipeline
    /// definition. If that is not found, and there is only one list element
    /// in the setup string, we assert that this is the element to handle.
    ///
    /// If `which` is not the empty string, we look for a list element with
    /// that name, and handle that either as a pipeline definition, or as a
    /// single operator definition.
    ///
    /// # Returns
    ///
    /// `true` on success, `false` on sseccus.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use geodesy::OperatorArgs;
    ///
    /// let mut args = OperatorArgs::global_defaults();
    /// let txt = std::fs::read_to_string("tests/tests.yml").unwrap_or_default();
    ///
    /// assert!(args.populate(&txt, "a_pipeline_for_testing"));
    /// assert_eq!(&args.value("_step_0", "")[0..4], "cart");
    /// ```
    ///
    ///
    pub fn populate(&mut self, definition: &str, which: &str) -> bool {
        // First, we copy the full text in the args, to enable recursive definitions
        self.insert("_definition", definition);

        // Read the entire YAML-document and try to locate the `which` document
        let docs = YamlLoader::load_from_str(definition).unwrap();
        let mut index = Some(0_usize);

        if which != "" {
            index = docs.iter().position(|doc| !doc[which].is_badvalue());
            if index.is_none() {
                return self.badvalue("Cannot locate definition");
            }
        }
        let index = index.unwrap();
        let main = &docs[index].as_hash();
        if main.is_none() {
            return self.badvalue("Cannot parse definition");
        }
        let main = main.unwrap();

        // Is it conforming?
        let mut main_entry_name = which;
        if main_entry_name.is_empty() {
            for (arg, val) in main {
                if val.is_badvalue() {
                    return self.badvalue("Cannot parse definition");
                }
                let name = &arg.as_str().unwrap();
                if name.starts_with('_') {
                    continue;
                }
                if !main_entry_name.is_empty() {
                    return self.badvalue("Too many items in definition root");
                }
                main_entry_name = name;
            }
        }
        self.name = main_entry_name.to_string();

        // Grab the sub-tree defining the 'main_entry_name'
        let main_entry = &docs[index][main_entry_name];
        if main_entry.is_badvalue() {
            return self.badvalue("Cannot locate definition");
        }

        // Loop over all globals and create the corresponding OperatorArgs entries
        if let Some(globals) = main_entry["globals"].as_hash() {
            for (arg, val) in globals {
                let thearg = arg.as_str().unwrap();
                if thearg != "inv" {
                    let theval = match val {
                        Yaml::Integer(val) => val.to_string(),
                        Yaml::Real(val) => val.as_str().to_string(),
                        Yaml::String(val) => val.to_string(),
                        Yaml::Boolean(val) => val.to_string(),
                        _ => "".to_string(),
                    };
                    if !theval.is_empty() {
                        self.insert(thearg, &theval);
                    }
                }
            }
        }

        // Try to locate the step definitions, to determine whether we
        // are handling a pipeline or a plain operator definition
        let steps = main_entry["steps"].as_vec();

        // Not a pipeline? Just insert the operator args and return
        if steps.is_none() {
            let args = main_entry.as_hash();
            if args.is_none() {
                return self.badvalue("Cannot read args");
            }
            let args = args.unwrap();
            for (arg, val) in args {
                let thearg = arg.as_str().unwrap();
                let theval = match val {
                    Yaml::Integer(val) => val.to_string(),
                    Yaml::Real(val) => val.as_str().to_string(),
                    Yaml::String(val) => val.to_string(),
                    Yaml::Boolean(val) => val.to_string(),
                    _ => "".to_string(),
                };
                if !theval.is_empty() {
                    self.insert(thearg, &theval);
                }
            }
            return true;
        }

        // It's a pipeline - insert the number of steps into the argument list.
        let steps = steps.unwrap();
        self.insert("_nsteps", &steps.len().to_string());

        // Insert each step into the argument list, formatted as YAML.
        // Unfortunately the compact mode does not work.
        for (index, step) in steps.iter().enumerate() {
            // Write the step definition to a new string
            let mut step_definition = String::new();
            let mut emitter = YamlEmitter::new(&mut step_definition);
            // emitter.compact(true);
            assert_eq!(emitter.is_compact(), true);
            emitter.dump(step).unwrap();

            // Remove the initial doc separator "---\n"
            let stripped_definition = step_definition.trim_start_matches("---\n");
            let step_key = format!("_step_{}", index);
            self.insert(&step_key, stripped_definition);
        }

        true
    }

    fn badvalue(&mut self, cause: &str) -> bool {
        self.name = "badvalue".to_string();
        self.insert("cause", cause);
        false
    }

    pub fn name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    pub fn insert(&mut self, key: &str, value: &str) {
        self.args.insert(key.to_string(), value.to_string());
    }

    pub fn append(&mut self, additional: &OperatorArgs) {
        let iter = additional.args.iter();
        for (key, val) in iter {
            self.insert(key, val);
        }
    }

    // Workhorse for ::value - this indirection is needed in order to keep the
    // original key available, when traversing an indirect definition.
    fn value_recursive_search(&mut self, key: &str, default: &str) -> String {
        let arg = self.args.get(key);
        let arg = match arg {
            Some(arg) => arg.to_string(),
            None => return default.to_string(),
        };
        // all_used includes intermediate steps in indirect definitions
        self.all_used.insert(key.to_string(), arg.to_string());

        if let Some(arg) = arg.strip_prefix("^") {
            return self.value_recursive_search(arg, default);
        }
        arg
    }

    /// Return the arg for a given key; maintain usage info.
    pub fn value(&mut self, key: &str, default: &str) -> String {
        let arg = self.value_recursive_search(key, default);
        if arg != default {
            self.used.insert(key.to_string(), arg.to_string());
        }
        arg
    }

    pub fn numeric_value(
        &mut self,
        operator_name: &str,
        key: &str,
        default: f64,
    ) -> Result<f64, String> {
        let arg = self.value(key, "");

        // key not given: return default
        if arg.is_empty() {
            return Ok(default);
        }

        // key given, value numeric: return value
        if let Ok(v) = arg.parse::<f64>() {
            return Ok(v);
        }

        // key given, but not numeric: return error string
        Err(format!(
            "Numeric value expected for '{}.{}' - got [{}: {}].",
            operator_name, key, key, arg
        ))
    }

    // If key is given, and value != false: true; else: false
    pub fn flag(&mut self, key: &str) -> bool {
        self.value(key, "false") != "false"
    }
}

//----------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #[test]
    fn operator_args() {
        use super::*;
        let mut args = OperatorArgs::new();

        // dx and dy are straightforward
        args.insert("dx", "11");
        args.insert("dy", "22");

        // But we hide dz behind two levels of indirection
        args.insert("dz", "^ddz");
        args.insert("ddz", "^dddz");
        args.insert("dddz", "33");
        // println!("args: {:?}", args);

        assert_eq!("00", args.value("", "00"));
        assert_eq!("11", args.value("dx", ""));
        assert_eq!("22", args.value("dy", ""));
        assert_eq!(args.used.len(), 2);

        assert_eq!("33", args.value("dz", ""));
        assert_eq!(33.0, args.numeric_value("foo", "dz", 42.0).unwrap());
        assert_eq!(42.0, args.numeric_value("foo", "bar", 42.0).unwrap());

        assert_eq!(args.used.len(), 3);
        assert_eq!(args.all_used.len(), 5);

        // println!("used: {:?}", &args.used);
        // println!("all_used: {:?}", &args.all_used);

        assert_eq!("", args.value("abcdefg", ""));

        // Finally one for testing 'err' returned for non-numerics
        args.insert("ds", "foo");
        assert!(args.numeric_value("bar", "ds", 0.0).is_err());
        // if let Err(msg) = args.numeric_value("bar", "ds", 0.0) {
        //     println!("**** err: {}", msg)
        // }
    }

    #[test]
    fn preparing_args() {
        use super::*;
        let mut args = OperatorArgs::global_defaults();

        // Explicitly stating the name of the pipeline
        let txt = std::fs::read_to_string("tests/tests.yml").unwrap_or_default();
        assert!(args.populate(&txt, "a_pipeline_for_testing"));
        assert_eq!(&args.value("_step_0", "    ")[0..4], "cart");

        // Let populate() figure out what we want
        let mut args = OperatorArgs::global_defaults();
        assert!(args.populate(&txt, ""));
        assert_eq!(&args.value("x", "5"), "3");

        // When op is not a pipeline
        let mut args = OperatorArgs::global_defaults();
        assert!(args.populate("cart: {ellps: intl}", ""));
        assert_eq!(args.name, "cart");
        assert_eq!(&args.value("ellps", ""), "intl");
    }

    #[test]
    fn bad_value() {
        use super::*;
        let v = Yaml::BadValue;
        assert!(v.is_badvalue());
        let v = Yaml::Null;
        assert!(v.is_null());
        let v = Yaml::Integer(77);
        assert!(v == Yaml::Integer(77));
    }
}
