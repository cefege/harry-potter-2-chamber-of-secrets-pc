//! Hosts `hp2-ui` menus inside the stock UWindow pages: the toolkit lays out, handles input and
//! produces draw commands; the page's own script functions do the canvas work, so clipping and GUI
//! scale stay exactly the stock page's.
use std::{cell::RefCell, collections::HashMap, ptr, rc::Rc};

use hp2_abi::engine::generated::{TArrayRaw, UClass, UObject, UProperty};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use hp2_ui::{DrawCmd, Fit, Font, Rect, TextAlign, TextMetrics};

use crate::{stock_graphics_menu::{live, object, prop}, ui_vm::{invoke, V}};

pub(crate) fn name_of(a: CoreAccess<'_>, object: *mut UObject) -> String {
    a.name_text(i32::from_ne_bytes(unsafe { (*object).name })).map(|u| u.iter().take_while(|&&c| c != 0).filter_map(|&c| char::from_u32(c)).collect()).unwrap_or_default()
}
pub(crate) fn class_name_of(a: CoreAccess<'_>, object: *mut UObject) -> String { name_of(a, unsafe { (*object).class.cast() }) }

pub(crate) unsafe fn find_class(a: CoreAccess<'_>, path: &str) -> RuntimeResult<Option<*mut UClass>> {
    Ok(unsafe { a.find_object_by_name(a.native_class("Core.Class")?, &path.chars().map(u32::from).collect::<Vec<_>>())? }.map(|p| p.cast()))
}

pub(crate) unsafe fn find_texture(a: CoreAccess<'_>, path: &str) -> RuntimeResult<*mut UObject> {
    let texture = unsafe { a.find_object_by_name(a.native_class("Engine.Texture")?, &path.chars().map(u32::from).collect::<Vec<_>>())? };
    texture.map(|p| p.cast()).ok_or_else(|| RuntimeError::new(&format!("Texture {path} not found")))
}

/// A string property of `object` (for example `UWindowComboListItem.Value`).
pub(crate) unsafe fn read_string(a: CoreAccess<'_>, object: *mut UObject, name: &str) -> RuntimeResult<String> {
    let array = unsafe { prop(a, object, name, "Core.StrProperty", size_of::<TArrayRaw>())?.cast::<TArrayRaw>().read_unaligned() };
    if array.num <= 0 || array.data.is_null() || array.num > 4096 {
        return Ok(String::new());
    }
    let units = unsafe { std::slice::from_raw_parts(array.data.cast::<u32>(), array.num as usize) };
    Ok(units.iter().take_while(|&&c| c != 0).filter_map(|&c| char::from_u32(c)).collect())
}

/// `Root.Fonts[index]`.
unsafe fn root_font(a: CoreAccess<'_>, root: *mut UObject, index: usize) -> RuntimeResult<*mut UObject> {
    let class = unsafe { (*root).class.cast::<UClass>() };
    let property = unsafe { a.find_field(class.cast(), a.intern_str("Fonts", FindName::Find)?, a.native_class("Core.ObjectProperty")?) }.cast::<UProperty>();
    if property.is_null() {
        return Err(RuntimeError::new("Root has no Fonts"));
    }
    let offset = usize::try_from(unsafe { (*property).offset }).map_err(|_| RuntimeError::new("Fonts offset"))?;
    if index >= usize::try_from(unsafe { (*property).array_dim }).unwrap_or(0) {
        return Err(RuntimeError::new("Font index out of range"));
    }
    Ok(unsafe { root.cast::<u8>().add(offset + index * size_of::<*mut UObject>()).cast::<*mut UObject>().read_unaligned() })
}

/// Text measurements learned during painting, shared with the toolkit's layout.
#[derive(Clone, Default)]
pub(crate) struct HostMetrics {
    cache: Rc<RefCell<HashMap<(String, Font), (f32, f32)>>>,
}

impl HostMetrics {
    pub(crate) fn knows(&self, text: &str, font: Font) -> bool {
        self.cache.borrow().contains_key(&(text.to_owned(), font))
    }
    pub(crate) fn learn(&self, text: &str, font: Font, size: (f32, f32)) {
        self.cache.borrow_mut().insert((text.to_owned(), font), size);
    }
}

impl TextMetrics for HostMetrics {
    fn text_size(&self, text: &str, font: Font) -> (f32, f32) {
        // Before the first paint has measured a string, use a plausible size.
        self.cache.borrow().get(&(text.to_owned(), font)).copied().unwrap_or((text.chars().count() as f32 * 7.5, 17.))
    }
}

pub(crate) fn font_index(font: Font) -> usize {
    match font {
        Font::Normal => 0,
        Font::Bold | Font::Heading => 1,
        Font::Slot(n) => usize::from(n),
    }
}

/// A page being painted: the window whose script functions draw, and the canvas.
pub(crate) struct Surface {
    pub page: *mut UObject,
    pub canvas: *mut UObject,
    root: *mut UObject,
    white: *mut UObject,
    textures: Rc<RefCell<HashMap<String, (*mut UObject, i32)>>>,
}

impl Surface {
    pub(crate) unsafe fn new(a: CoreAccess<'_>, page: *mut UObject, canvas: *mut UObject, textures: Rc<RefCell<HashMap<String, (*mut UObject, i32)>>>) -> RuntimeResult<Self> {
        let root = unsafe { object(a, page, "Root")? };
        if !live(a, root) || !live(a, canvas) {
            return Err(RuntimeError::new("UI surface unavailable"));
        }
        let mut surface = Self { page, canvas, root, white: ptr::null_mut(), textures };
        surface.white = unsafe { surface.texture(a, "UWindow.WhiteTexture")? };
        Ok(surface)
    }

    /// A texture by name, cached while the object stays live.
    unsafe fn texture(&self, a: CoreAccess<'_>, name: &str) -> RuntimeResult<*mut UObject> {
        // `#<index>` names a live object directly (textures read from stock controls).
        if let Some(index) = name.strip_prefix('#').and_then(|i| i.parse::<i32>().ok()) {
            let object = a.object_at(index).cast::<UObject>();
            if object.is_null() {
                return Err(RuntimeError::new("Texture object is gone"));
            }
            return Ok(object);
        }
        if let Some(&(object, index)) = self.textures.borrow().get(name)
            && a.object_at(index) == object.cast()
        {
            return Ok(object);
        }
        let object = unsafe { find_texture(a, name)? };
        self.textures.borrow_mut().insert(name.to_owned(), (object, unsafe { (*object).index }));
        Ok(object)
    }

    unsafe fn set_color(&self, a: CoreAccess<'_>, color: [u8; 4]) -> RuntimeResult<()> {
        unsafe { prop(a, self.canvas, "DrawColor", "Core.StructProperty", 4)?.cast::<[u8; 4]>().write_unaligned(color) };
        Ok(())
    }

    unsafe fn use_font(&self, a: CoreAccess<'_>, font: Font, color: [u8; 4]) -> RuntimeResult<()> {
        let font = unsafe { root_font(a, self.root, font_index(font))? };
        unsafe { prop(a, self.canvas, "Font", "Core.ObjectProperty", size_of::<*mut UObject>())?.cast::<*mut UObject>().write_unaligned(font) };
        unsafe { self.set_color(a, color) }
    }

    /// Text width and height in page units.
    pub(crate) unsafe fn text_size(&self, a: CoreAccess<'_>, text: &str, font: Font) -> RuntimeResult<(f32, f32)> {
        unsafe { self.use_font(a, font, [255, 255, 255, 255])? };
        let outs = unsafe { invoke(a, self.page, "TextSize", &[("C", V::Obj(self.canvas)), ("Text", V::S(text))], &["W", "H"], ptr::null_mut())? };
        Ok((outs[0], outs[1]))
    }

    unsafe fn fill(&self, a: CoreAccess<'_>, rect: Rect, color: [u8; 4]) -> RuntimeResult<()> {
        unsafe { self.set_color(a, color)? };
        unsafe { invoke(a, self.page, "DrawStretchedTexture", &[("C", V::Obj(self.canvas)), ("X", V::F(rect.x)), ("Y", V::F(rect.y)), ("W", V::F(rect.w)), ("H", V::F(rect.h)), ("Tex", V::Obj(self.white))], &[], ptr::null_mut())? };
        Ok(())
    }

    /// Draws a toolkit draw list. `metrics` must already know every drawn string.
    pub(crate) unsafe fn render(&self, a: CoreAccess<'_>, commands: &[DrawCmd], metrics: &HostMetrics) -> RuntimeResult<()> {
        let mut clips: Vec<Rect> = Vec::new();
        for command in commands {
            match command {
                DrawCmd::Rect { rect, color } => unsafe { self.fill(a, *rect, [color.0, color.1, color.2, color.3])? },
                DrawCmd::Outline { rect, color } => {
                    let c = [color.0, color.1, color.2, color.3];
                    for edge in [Rect::new(rect.x, rect.y, rect.w, 1.), Rect::new(rect.x, rect.bottom() - 1., rect.w, 1.), Rect::new(rect.x, rect.y, 1., rect.h), Rect::new(rect.right() - 1., rect.y, 1., rect.h)] {
                        unsafe { self.fill(a, edge, c)? };
                    }
                }
                DrawCmd::Image { texture, rect, fit } => {
                    let tex = unsafe { self.texture(a, &texture.0)? };
                    unsafe { self.set_color(a, [255, 255, 255, 255])? };
                    let size = unsafe { (i32_prop(a, tex, "USize") as f32, i32_prop(a, tex, "VSize") as f32) };
                    // Destination rectangle and the texture region drawn into it.
                    let (dest, src) = match fit {
                        Fit::Native => (Rect::new(rect.x, rect.y, size.0, size.1), Rect::new(0., 0., size.0, size.1)),
                        Fit::Stretch => (*rect, Rect::new(0., 0., size.0, size.1)),
                        Fit::Region(u, v, w, h) => (*rect, Rect::new(*u, *v, *w, *h)),
                    };
                    let Some((dest, src)) = clip_image(dest, src, clips.last().copied()) else { continue };
                    unsafe { invoke(a, self.page, "DrawStretchedTextureSegment", &[("C", V::Obj(self.canvas)), ("X", V::F(dest.x)), ("Y", V::F(dest.y)), ("W", V::F(dest.w)), ("H", V::F(dest.h)), ("tX", V::F(src.x)), ("tY", V::F(src.y)), ("tW", V::F(src.w)), ("tH", V::F(src.h)), ("Tex", V::Obj(tex))], &[], ptr::null_mut())? };
                }
                DrawCmd::Text { rect, text, font, color, align } => {
                    if text.is_empty() {
                        continue;
                    }
                    let (tw, th) = metrics.text_size(text, *font);
                    let x = match align {
                        TextAlign::Left => rect.x,
                        TextAlign::Center => rect.x + (rect.w - tw) / 2.,
                        TextAlign::Right => rect.right() - tw,
                    };
                    let y = rect.y + (rect.h - th) / 2.;
                    unsafe { self.use_font(a, *font, [color.0, color.1, color.2, color.3])? };
                    unsafe { invoke(a, self.page, "ClipText", &[("C", V::Obj(self.canvas)), ("X", V::F(x)), ("Y", V::F(y)), ("S", V::S(text))], &[], ptr::null_mut())? };
                }
                DrawCmd::PushClip(rect) => {
                    let next = clips.last().map_or(*rect, |outer| intersect(*outer, *rect));
                    clips.push(next);
                }
                DrawCmd::PopClip => {
                    clips.pop();
                }
            }
        }
        Ok(())
    }
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    Rect::new(x, y, (a.right().min(b.right()) - x).max(0.), (a.bottom().min(b.bottom()) - y).max(0.))
}

/// Crops `dest` to `clip` and the texture region `src` in proportion. `None` when nothing is left.
fn clip_image(dest: Rect, src: Rect, clip: Option<Rect>) -> Option<(Rect, Rect)> {
    let Some(clip) = clip else { return Some((dest, src)) };
    if dest.w <= 0. || dest.h <= 0. {
        return None;
    }
    let cut = intersect(dest, clip);
    if cut.w <= 0. || cut.h <= 0. {
        return None;
    }
    let (sx, sy) = (src.w / dest.w, src.h / dest.h);
    Some((cut, Rect::new(src.x + (cut.x - dest.x) * sx, src.y + (cut.y - dest.y) * sy, cut.w * sx, cut.h * sy)))
}

unsafe fn i32_prop(a: CoreAccess<'_>, o: *mut UObject, name: &str) -> i32 {
    unsafe { prop(a, o, name, "Core.IntProperty", 4) }.map(|p| unsafe { p.cast::<i32>().read_unaligned() }).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipping_crops_the_texture_region_in_proportion() {
        let (d, s) = clip_image(Rect::new(10., 10., 256., 256.), Rect::new(0., 0., 256., 256.), Some(Rect::new(10., 10., 180., 24.))).unwrap();
        assert_eq!((d, s), (Rect::new(10., 10., 180., 24.), Rect::new(0., 0., 180., 24.)));
        let (d, s) = clip_image(Rect::new(0., 0., 20., 20.), Rect::new(0., 0., 10., 10.), Some(Rect::new(5., 0., 100., 100.))).unwrap();
        assert_eq!((d, s), (Rect::new(5., 0., 15., 20.), Rect::new(2.5, 0., 7.5, 10.)));
        assert!(clip_image(Rect::new(0., 0., 5., 5.), Rect::new(0., 0., 5., 5.), Some(Rect::new(10., 10., 5., 5.))).is_none());
    }
}
