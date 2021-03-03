use std::rc::Rc;

use xmlparser;

macro_rules! attribute_scanner {
    ($name:ident, $($attr:ident),+) => {
        pub fn $name(buf: &[u8]) -> Result<Box<dyn Iterator<Item=Rc<str>>>, Box<dyn std::error::Error>> {
            let maybe_str = std::str::from_utf8(buf);
            let buf_str = match maybe_str {
                Ok(s) => s,
                Err(e) => return Err(Box::new(e))
            };
            let tokens = xmlparser::Tokenizer::from_fragment(buf_str, 0..(buf_str.len()));
            let mut res = Vec::<Rc<str>>::new();
            for tok in tokens {
                if let Ok(xmlparser::Token::Attribute { local, value, .. }) = tok {
                    let name = local.as_str();
                    if true $(|| name == stringify!($attr))+ {
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

attribute_scanner!(scan_object, name, culling_object, default_material, file, object, materials);
attribute_scanner!(scan_animation_state_machine, file);
attribute_scanner!(scan_animation_subset, file);
attribute_scanner!(scan_effect, texture, material_config, model, object, effect);