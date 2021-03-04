use std::rc::Rc;

use xmlparser;

type DynResult<TOk> = Result<TOk, Box<dyn std::error::Error>>;
type TryStringIterator = DynResult<Box<dyn Iterator<Item=Rc<str>>>>;

#[derive(Debug)]
struct XmlNestError {
    expected: Rc<str>,
    got: Rc<str>
}
impl std::fmt::Display for XmlNestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Incorrect nesting. Expected '{}', got '{}'", self.expected, self.got)
    }
}
impl std::error::Error for XmlNestError { }

fn tokenise(buf: &[u8]) -> DynResult<xmlparser::Tokenizer> {
    let maybe_str = std::str::from_utf8(buf);
    let buf_str = match maybe_str {
        Ok(s) => s,
        Err(e) => return Err(Box::new(e))
    };
    let tokens = xmlparser::Tokenizer::from_fragment(buf_str, 0..(buf_str.len()));
    return Ok(tokens);
}

macro_rules! attribute_scanner {
    ($name:ident, $($attr:ident),+) => {
        pub fn $name(buf: &[u8]) -> TryStringIterator {
            scan_by_attributes(buf, |_, attname, value, res| {
                if false $(|| attname == stringify!($attr))+ {
                    res.push(Rc::from(value));
                }
            })
        }
    }
}

attribute_scanner!(scan_animation_state_machine, file);
attribute_scanner!(scan_animation_subset, file);
attribute_scanner!(scan_effect, texture, material_config, model, object, effect);

pub fn scan_animation_def(buf: &[u8]) -> TryStringIterator {
    //xpath: //bone/@name | //subset/@file
    scan_by_attributes(buf, |stack, attname, value, res| {
        let ce = *stack.last().unwrap();
        if (ce == "bone" && attname == "name") || (ce == "subset" && attname == "file") {
            res.push(Rc::from(value))
        }
    })
}

pub fn scan_object(buf: &[u8]) -> TryStringIterator {
    //xpath: //@name | //@culling_object | //@default_material | //@file | //@object
    //       | /diesel/@materials | split(//@materials, ",")
    scan_by_attributes(buf, |stack, attname, value, res| {
        let ce = *stack.last().unwrap();
        if attname=="name"
            ||attname=="culling_object"
            ||attname=="default_material"
            ||attname=="file"
            ||attname=="object"
        {
            res.push(Rc::from(value));
        }

        if attname == "materials" && ce == "diesel" {
            res.push(Rc::from(value));
        }

        if attname == "materials" && ce != "diesel" {
            res.extend(value.split(",").map(str::trim).map(Rc::from));
        }
    })
}

pub fn scan_scene(buf: &[u8]) -> TryStringIterator {
    //xpath: //load_scene/@file | //load_scene/@materials | //object/@name
    scan_by_attributes(buf, |stack, attname, value, res| {
        let ce = *stack.last().unwrap();
        if (ce == "load_scene" && attname == "file")
        || (ce == "load_scene" && attname == "materials")
        || (ce == "object"     && attname == "name")
        {
            res.push(Rc::from(value))
        }
    })
}


pub fn scan_gui(buf: &[u8]) -> TryStringIterator {
    //xpath: @font_s | @font | //bitmap/@texture_s | //preload/@texture
    scan_by_attributes(buf, |stack, attname, value, res| {
        let ce = *stack.last().unwrap();
        if (attname == "font_s")
        || (attname == "font")
        || (ce == "bitmap" && attname == "texture_s")
        || (ce == "preload" && attname == "texture")
        {
            res.push(Rc::from(value))
        }
    })
}

pub fn scan_merged_font(buf: &[u8]) -> TryStringIterator {
    //xpath: /merged_font/font/@name
    scan_by_attributes(buf, |stack, attname, value, res| {
        if stack.get(0) == Some(&"merged_font")
        && stack.get(1) == Some(&"font")
        && attname == "name" {
            res.push(Rc::from(value));
        }
    })
}

pub fn scan_material_config(buf: &[u8]) -> TryStringIterator {
    //xpath: /materials/@group | /materials/material/@name | //@file
    scan_by_attributes(buf, |stack, attname, value, res| {
        if attname == "file"
        || (stack.get(0) == Some(&"materials") && attname == "group")
        || (stack.get(0) == Some(&"materials") && stack.get(1) == Some(&"material") && attname == "name")
        {
            res.push(Rc::from(value));
        }
    })
}

pub fn scan_unit(buf: &[u8]) -> TryStringIterator {
    /* XPath:
        /unit/anim_state_machine/@name | /unit/object/@file | /unit/network/@remote_unit
      | /unit/extensions/extension[@class='CopDamage']/var[@name='_head_gear']/@value
      | /unit/extensions/extension[@class='CopDamage']/var[@name='_head_gear_object']/@value
      | /unit/extensions/extension[@class='CopDamage']/var[@name='_head_gear_decal_mesh']/@value
      | /unit/dependencies/depends_on/attribute::*
    */

    let tokens = tokenise(buf)?;
    let mut res = Vec::<Rc<str>>::new();
    let mut elem_stack = Vec::<&str>::with_capacity(4);

    let mut capture_extension = false;
    let mut capture_var = false;
    let mut value_to_capture = None::<&str>;

    for tok in tokens {
        use xmlparser::Token::*;
        match tok {
            Err(e) => return Err(Box::new(e)),
            Ok(ElementStart{local, ..}) => elem_stack.push(local.as_str()),
            Ok(ElementEnd{end: xmlparser::ElementEnd::Empty, ..}) => { elem_stack.pop(); },
            Ok(ElementEnd{end: xmlparser::ElementEnd::Close(_, tn), ..}) => {
                try_pop_element(&mut elem_stack, tn)?;

                if capture_extension && capture_var {
                    if let Some(value) = value_to_capture {
                        res.push(Rc::from(value));
                    }
                }

                if elem_stack.len() == 3 {
                    value_to_capture = None;
                    capture_var = false;
                }
                if elem_stack.len() == 2 {
                    capture_extension = false;
                }
            },
            Ok(Attribute{local, value, ..}) => {
                let attname = local.as_str();
                if elem_stack.get(0) != Some(&"unit") { continue; }

                if (elem_stack.len() == 2 && elem_stack.get(1) == Some(&"anim_state_machine") && attname == "name")
                || (elem_stack.len() == 2 && elem_stack.get(1) == Some(&"object") && attname == "file")
                || (elem_stack.len() == 2 && elem_stack.get(1) == Some(&"network") && attname == "remote_unit")
                {
                    res.push(Rc::from(value.as_str()));
                }

                if elem_stack.len() == 3 && elem_stack.get(1) == Some(&"dependencies")
                && elem_stack.get(2) == Some(&"depends_on") {
                    res.push(Rc::from(value.as_str()));
                }

                if elem_stack.len() == 3 && elem_stack.get(1) == Some(&"extensions")
                && elem_stack.get(2) == Some(&"extension")
                && attname == "class" && value.as_str() == "CopDamage" {
                    capture_extension = true;
                }

                if elem_stack.len() == 3 && elem_stack.get(1) == Some(&"extensions")
                && elem_stack.get(2) == Some(&"extension")
                && elem_stack.get(3) == Some(&"var")
                && attname == "var"
                && (
                    value.as_str() == "_head_gear" 
                    || value.as_str() == "_head_gear_object"
                    || value.as_str() == "_head_gear_decal_mesh"
                ) {
                    capture_var = true;
                }

                if attname == "value" {
                    value_to_capture = Some(value.as_str())
                }

            }
            _ => ()
        }
    }

    Ok(Box::new(res.into_iter()))
}

fn scan_by_attributes<F>(buf: &[u8], mapper: F) -> TryStringIterator
where
    F: Fn(&[&str], &str, &str, &mut Vec<Rc<str>>)
{
    let tokens = tokenise(buf)?;
    let mut res = Vec::<Rc<str>>::new();
    let mut elem_stack = Vec::<&str>::with_capacity(4);
    for tok in tokens {
        use xmlparser::Token::*;
        match tok {
            Err(e) => return Err(Box::new(e)),
            Ok(ElementStart{local, ..}) => elem_stack.push(local.as_str()),
            Ok(ElementEnd{end: xmlparser::ElementEnd::Empty, ..}) => { elem_stack.pop(); },
            Ok(ElementEnd{end: xmlparser::ElementEnd::Close(_, tn), ..}) => {
                try_pop_element(&mut elem_stack, tn)?;
            },
            Ok(Attribute{local, value, ..}) => {
                let attname = local.as_str();
                mapper(&elem_stack, attname, value.as_str(), &mut res);
            }
            _ => ()
        }
    }

    Ok(Box::new(res.into_iter()))
}

fn try_pop_element(stack: &mut Vec<&str>, expected: xmlparser::StrSpan) -> DynResult<()> {
    if let Some(top) = stack.last() {
        if *top == expected.as_str() {
            stack.pop();
            return Ok(());
        }
        else {
            return Err(Box::new(XmlNestError {
                expected: Rc::from(expected.as_str()),
                got: Rc::from(*top)
            }))
        }
    }
    else {
        return Err(Box::new(XmlNestError {
            got: Rc::from("(document)"),
            expected: Rc::from(expected.as_str())
        }))
    }
}