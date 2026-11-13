//! House-score plaques own a complete view; NotifyActor setup, timers and status creation stay script-owned.
use std::{cell::RefCell, ptr};
use hp2_abi::engine::generated::{UFont, UFunction, UObject, UScriptedTexture};
use hp2_core::{ScriptPreCallDecision, core_access::{CoreAccess, RuntimeError, RuntimeResult}};
use hp2_ui::{BlendMode, Color, Font, Node, Rect, Signal, Size, Store, Ui, col, label};
use super::{find_function, int_of, param_object};
use crate::{
    stock_graphics_menu::{Weak, is_a, live, object, prop},
    ui_console_views::{declared, need, parameter, resolved},
    ui_host::{DrawTarget, HostMetrics, find_class, render_commands},
    ui_vm::{self, V},
};

const HOUSES: [(&str, &str); 4] = [
    ("stexGryffindor", "HGame.StatusItemGryffindorPts"),
    ("stexSlytherin", "HGame.StatusItemSlytherinPts"),
    ("stexHufflepuff", "HGame.StatusItemHufflePuffPts"),
    ("stexRavenclaw", "HGame.StatusItemRavenclawPts"),
];
#[derive(Clone, Debug, PartialEq)]
struct Score { text: String, at: (f32, f32), extent: (f32, f32), size: (i32, i32), color: [u8; 4] }
struct Retained { owner: Weak, texture: Weak, model: Signal<Score>, ui: Ui, metrics: HostMetrics }
thread_local! { static VIEWS: RefCell<Vec<Retained>> = const { RefCell::new(Vec::new()) }; }
fn view(score: &Score) -> Node {
    let c = score.color;
    col().children([label(&score.text).font(Font::Slot(0)).color(Color(c[0], c[1], c[2], c[3]))
        .at(score.at.0, score.at.1).width(Size::Px(score.extent.0)).height(Size::Px(score.extent.1))])
}
fn centered(midpoint: i32, extent: f32) -> f32 { (midpoint as f32 - extent / 2.) as i32 as f32 }
struct Target { texture: *mut UObject, font: *mut UFont }
impl DrawTarget for Target {
    unsafe fn fill(&self, _: CoreAccess<'_>, _: Rect, _: [u8; 4]) -> RuntimeResult<()> { Err(RuntimeError::new("Unexpected Ceremony fill")) }
    unsafe fn texture_object(&self, _: CoreAccess<'_>, _: &str) -> RuntimeResult<*mut UObject> { Err(RuntimeError::new("Unexpected Ceremony image")) }
    unsafe fn segment(&self, _: CoreAccess<'_>, _: *mut UObject, _: Rect, _: Rect, _: [u8; 4], _: BlendMode) -> RuntimeResult<()> { Err(RuntimeError::new("Unexpected Ceremony image")) }
    unsafe fn text_at(&self, a: CoreAccess<'_>, x: f32, y: f32, text: &str, _: Font, color: [u8; 4]) -> RuntimeResult<()> {
        let units: Vec<u32> = text.chars().map(u32::from).chain([0]).collect();
        unsafe { crate::scripted_texture_owner::draw_colored_text(a, self.texture, ptr::null_mut(), x, y, units.as_ptr().cast_mut().cast(), units.len(), self.font, u32::from_ne_bytes(color)) }
    }
}
unsafe fn house(a: CoreAccess<'_>, receiver: *mut UObject, texture: *mut UObject) -> RuntimeResult<Option<usize>> {
    unsafe {
        for (index, (name, _)) in HOUSES.iter().enumerate() {
            if object(a, receiver, name)? == texture { return Ok(Some(index)); }
        }
        Ok(None)
    }
}
unsafe fn query_layout(a: CoreAccess<'_>, function: *mut UFunction, class_parameter: &str) -> RuntimeResult<()> {
    unsafe {
        if function.is_null() || (*function).i_native != 0 || (*function).function_flags & 0x400 != 0 {
            return Err(RuntimeError::new("Ceremony status query is not a script function"));
        }
        parameter(a, function, class_parameter, "Core.ClassProperty", size_of::<*mut UObject>())?;
        parameter(a, function, "ReturnValue", "Core.ObjectProperty", size_of::<*mut UObject>())?;
        Ok(())
    }
}
struct Prepared { manager: *mut UObject, font: *mut UFont }
unsafe fn preflight(a: CoreAccess<'_>, receiver: *mut UObject, texture: *mut UObject) -> RuntimeResult<Prepared> {
    unsafe {
        need(a, receiver)?; need(a, texture)?;
        if !is_a(a, texture, a.native_class("Engine.ScriptedTexture")?) {
            return Err(RuntimeError::new("Ceremony target is not a ScriptedTexture"));
        }
        if ptr::addr_of!((*texture.cast::<UScriptedTexture>()).palette_map).read_unaligned().is_null() {
            return Err(RuntimeError::new("Ceremony target palette cache unavailable"));
        }
        if house(a, receiver, texture)?.is_none() {
            return Err(RuntimeError::new("Unexpected Ceremony texture retains the stock warning route"));
        }
        for name in ["nScoreMidX", "nScoreMidY"] { int_of(a, receiver, name)?; }
        for name in ["USize", "VSize"] { int_of(a, texture, name)?; }
        prop(a, receiver, "colorScoreFont", "Core.StructProperty", 4)?;
        let level = object(a, receiver, "Level")?; need(a, level)?;
        let harry = object(a, level, "PlayerHarryActor")?; need(a, harry)?;
        let harry_class = find_class(a, "HGame.harry")?.ok_or_else(|| RuntimeError::new("Ceremony player class unavailable"))?;
        if !is_a(a, harry, harry_class) { return Err(RuntimeError::new("Ceremony player cast unavailable")); }
        let player = object(a, harry, "Player")?; need(a, player)?;
        let console = object(a, player, "Console")?; need(a, console)?;
        let console_class = find_class(a, "HGame.baseConsole")?.ok_or_else(|| RuntimeError::new("Ceremony console class unavailable"))?;
        if !is_a(a, console, console_class) { return Err(RuntimeError::new("Ceremony console cast unavailable")); }
        let font = object(a, console, "IntMedFont")?; need(a, font)?;
        if !is_a(a, font, a.native_class("Engine.Font")?) { return Err(RuntimeError::new("Ceremony font unsupported")); }
        let manager = object(a, harry, "managerStatus")?; need(a, manager)?;
        query_layout(a, resolved(a, manager, "GetStatusGroup")?, "classGroup")?;
        query_layout(a, declared(a, "HGame.StatusGroup.GetStatusItem")?, "classItem")?;
        for path in std::iter::once("HGame.StatusGroupHousePoints").chain(HOUSES.iter().map(|entry| entry.1)) {
            if find_class(a, path)?.is_none() { return Err(RuntimeError::new("Ceremony status class unavailable")); }
        }
        Ok(Prepared { manager, font: font.cast() })
    }
}
unsafe fn draw(a: CoreAccess<'_>, receiver: *mut UObject, texture: *mut UObject, prepared: Prepared) -> RuntimeResult<()> {
    unsafe {
        let group_class = find_class(a, "HGame.StatusGroupHousePoints")?.ok_or_else(|| RuntimeError::new("Ceremony group class unavailable"))?;
        let mut group: *mut UObject = ptr::null_mut();
        // These are the real, state-first script lookups, including item/group creation.
        ui_vm::invoke_virtual(a, prepared.manager, "GetStatusGroup", &[("classGroup", V::Obj(group_class.cast()))], &[], (&raw mut group).cast())?;
        need(a, receiver)?; need(a, group)?;
        let index = house(a, receiver, texture)?.ok_or_else(|| RuntimeError::new("Ceremony texture changed during status lookup"))?;
        let item_class = find_class(a, HOUSES[index].1)?.ok_or_else(|| RuntimeError::new("Ceremony item class unavailable"))?;
        let get_item = resolved(a, group, "GetStatusItem")?;
        query_layout(a, get_item, "classItem")?;
        let mut item: *mut UObject = ptr::null_mut();
        ui_vm::invoke_resolved(a, group, get_item, &[("classItem", V::Obj(item_class.cast()))], &[], (&raw mut item).cast())?;
        need(a, item)?;
        let text = int_of(a, item, "nCount")?.to_string();
        need(a, receiver)?; need(a, texture)?; need(a, prepared.font.cast())?;
        let units: Vec<u32> = text.chars().map(u32::from).chain([0]).collect();
        let extent = crate::scripted_texture_owner::text_size(a, texture, ptr::null_mut(), units.as_ptr().cast_mut().cast(), units.len(), prepared.font)?
            .ok_or_else(|| RuntimeError::new("Ceremony score measurement unavailable"))?;
        let score = Score {
            at: (centered(int_of(a, receiver, "nScoreMidX")?, extent.0), centered(int_of(a, receiver, "nScoreMidY")?, extent.1)),
            extent, size: (int_of(a, texture, "USize")?, int_of(a, texture, "VSize")?),
            color: prop(a, receiver, "colorScoreFont", "Core.StructProperty", 4)?.cast::<[u8; 4]>().read_unaligned(), text,
        };
        let mut retained = VIEWS.with(|views| {
            let mut views = views.borrow_mut();
            views.retain(|v| v.owner.live(a) && v.texture.live(a));
            views.iter().position(|v| v.owner.p == receiver && v.texture.p == texture).map(|index| views.swap_remove(index))
        }).unwrap_or_else(|| {
            let store = Store::new(); let model = store.signal(score.clone()); let observed = model.clone(); let metrics = HostMetrics::default();
            Retained { owner: Weak::new(receiver), texture: Weak::new(texture), model, ui: Ui::new(store, move || view(&observed.get()), metrics.clone()), metrics }
        });
        retained.metrics.clear(); retained.metrics.learn(&score.text, Font::Slot(0), extent);
        retained.ui.resize(score.size.0 as f32, score.size.1 as f32);
        retained.model.set(score); retained.ui.relayout();
        let result = render_commands(&Target { texture, font: prepared.font }, a, &retained.ui.draw(), &retained.metrics);
        if retained.owner.live(a) && retained.texture.live(a) { VIEWS.with(|views| views.borrow_mut().push(retained)); }
        result
    }
}
unsafe fn callback(a: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<ScriptPreCallDecision> {
    if !super::enabled() || !live(a, receiver) { return Ok(ScriptPreCallDecision::Continue); }
    unsafe {
        let Some(texture) = param_object(a, function, params, size, "Tex") else { return Ok(ScriptPreCallDecision::Continue); };
        let Ok(prepared) = preflight(a, receiver, texture) else { return Ok(ScriptPreCallDecision::Continue); };
        draw(a, receiver, texture, prepared)?;
        Ok(ScriptPreCallDecision::Consume)
    }
}
pub(super) unsafe fn install(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    if !super::enabled() { return Ok(None); }
    unsafe {
        let function = find_function(a, "HGame.CeremonySTextures", "Core.Class", "RenderTexture")?;
        if function.is_null() { return Ok(None); }
        Ok(Some(a.install_script_pre_observer("ui-ceremony", &[function], callback)?))
    }
}
#[cfg(test)]
pub(super) fn assert_view() {
    assert_eq!(centered(64, 17.), 55.);
    assert_eq!(centered(-2, 3.), -3.);
    let score = Score { text:"123".into(), at:(55., 24.), extent:(17., 16.), size:(128, 64), color:[9, 19, 29, 39] };
    let metrics = HostMetrics::default(); metrics.learn("123", Font::Slot(0), score.extent);
    let mut ui = Ui::new(Store::new(), move || view(&score), metrics); ui.resize(128., 64.); ui.relayout();
    assert!(ui.draw().iter().any(|command| matches!(command, hp2_ui::DrawCmd::Text { rect, text, color:Color(9,19,29,39), .. } if rect.x == 55. && rect.y == 24. && text == "123")));
}
