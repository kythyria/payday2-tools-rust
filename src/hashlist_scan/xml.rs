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
            let tokens = tokenise(buf)?;
            let mut res = Vec::<Rc<str>>::new();
            for tok in tokens {
                if let Ok(xmlparser::Token::Attribute { local, value, .. }) = tok {
                    let name = local.as_str();
                    if false $(|| name == stringify!($attr))+ {
                        res.push(Rc::from(value.as_str()));
                    }
                }
                else if let Err(e) = tok {
                    return Err(Box::new(e));
                }
            }
            Ok(Box::new(res.into_iter()))
        }
    }
}

//attribute_scanner!(scan_object, name, culling_object, default_material, file, object, materials);
attribute_scanner!(scan_animation_state_machine, file);
attribute_scanner!(scan_animation_subset, file);
attribute_scanner!(scan_effect, texture, material_config, model, object, effect);

pub fn scan_animation_def(buf: &[u8]) -> TryStringIterator {
    let tokens = tokenise(buf)?;
    let mut res = Vec::<Rc<str>>::new();

    //xpath: //bone/@name | //subset/@file

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
                let ce = *elem_stack.last().unwrap();
                if (ce == "bone" && local.as_str() == "name") || ce == "subset" && local.as_str() == "file" {
                    res.push(Rc::from(value.as_str()))
                }
            }
            _ => ()
        }
    }

    Ok(Box::new(res.into_iter()))
}

pub fn scan_object(buf: &[u8]) -> TryStringIterator {
    let tokens = tokenise(buf)?;
    let mut res = Vec::<Rc<str>>::new();

    //xpath: //@name | //@culling_object | //@default_material | //@file | //@object
    //       | /diesel/@materials | split(//@materials, ",")

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
                let name = local.as_str();
                let ce = *elem_stack.last().unwrap();
                if name=="name"
                    ||name=="culling_object"
                    ||name=="default_material"
                    ||name=="file"
                    ||name=="object"
                {
                    res.push(Rc::from(value.as_str()));
                }

                if name == "materials" && ce == "diesel" {
                    res.push(Rc::from(value.as_str()))
                }

                if name == "materials" && ce != "diesel" {
                    res.extend(value.as_str().split(",").map(str::trim).map(Rc::from));
                }
            }
            _ => ()
        }
    }

    Ok(Box::new(res.into_iter()))
}

pub fn scan_scene(buf: &[u8]) -> TryStringIterator {
    let tokens = tokenise(buf)?;
    let mut res = Vec::<Rc<str>>::new();

    //xpath: //load_scene/@file | //load_scene/@materials | //object/@name

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
                let ce = *elem_stack.last().unwrap();
                let attname = local.as_str();
                if (ce == "load_scene" && attname == "file")
                || (ce == "load_scene" && attname == "materials")
                || (ce == "object"     && attname == "name")
                {
                    res.push(Rc::from(value.as_str()))
                }
            }
            _ => ()
        }
    }

    Ok(Box::new(res.into_iter()))
}

pub fn scan_gui(buf: &[u8]) -> TryStringIterator {
    let tokens = tokenise(buf)?;
    let mut res = Vec::<Rc<str>>::new();

    //xpath: @font_s | @font | //bitmap/@texture_s | //preload/@texture

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
                let ce = *elem_stack.last().unwrap();
                let attname = local.as_str();
                if (attname == "font_s")
                || (attname == "font")
                || (ce == "bitmap" && attname == "texture_s")
                || (ce == "preload" && attname == "texture")
                {
                    res.push(Rc::from(value.as_str()))
                }
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