//! Canonical Core ownership of `ULevel::SpawnPlayActor` operations.

use crate::level_spawn_play_actor::{self, Backend, Op};
use hp2_abi::engine::generated::{AGameInfoLoginParams, AActor, ALevelInfo, APlayerPawn, FURL, TArrayRaw, UClass, ULevel, UNetConnection, UPackageMap, UObject, UProperty};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use std::{cell::RefCell, ffi::c_void, ptr};

const LOAD_NO_WARN: u32 = 0x0002;
const LOAD_ALLOW_DLL: u32 = 0x0008;
const ROLE_AUTHORITY: u8 = 4;
const SHOW_PLAYER_CTRL: u32 = 0x0000_0800;
const SHOW_REAL_TIME: u32 = 0x0000_4000;
const SHOW_BACKDROP: u32 = 0x0000_0004;
const SHOW_ACTORS: u32 = 0x0000_0008;
const REND_DYN_LIGHT: u8 = 2;
const CPF_TRAVEL: u32 = 0x0001_0000;
const CLASS_BUFFER: usize = 256;
const DEFAULT_USER: [u32; 27] = [117, 115, 114, 58, 68, 101, 102, 97, 117, 108, 116, 80, 108, 97, 121, 101, 114, 46, 67, 108, 97, 115, 115, 0, 0, 0, 0];
const DEFAULT_INI: [u32; 14] = [105, 110, 105, 58, 85, 82, 76, 46, 67, 108, 97, 115, 115, 0];

#[derive(Default)]
struct AcceptedActor {
    actor: *mut AActor,
    name: Vec<u32>,
    parameters: Vec<Vec<u32>>,
}

#[derive(Default)]
struct Scratch {
    options: Vec<u32>,
    class_option: Vec<u32>,
    load_option: Vec<u32>,
    player_name: Vec<u32>,
    class_name: Vec<u32>,
    actor_name: Vec<u32>,
    input: Vec<u32>,
    input_cursor: usize,
    value: Vec<u32>,
    travel_text: Vec<u32>,
    element: i32,
    data: *mut u8,
    property: *mut UProperty,
    iterator: *mut c_void,
    iterator_struct: *mut hp2_abi::engine::generated::UStruct,
    actor_index: usize,
    accepted: Vec<AcceptedActor>,
    loading: bool,
}

struct CoreSpawnPlayBackend<'a> {
    access: CoreAccess<'a>,
    level: *mut ULevel,
    player: *mut UObject,
    role: i32,
    url: *mut FURL,
    error: *mut TArrayRaw,
    package_map: std::cell::Cell<*mut UPackageMap>,
    player_class: std::cell::Cell<*mut UClass>,
    actor: std::cell::Cell<*mut APlayerPawn>,
    scratch: RefCell<Scratch>,
}

impl CoreSpawnPlayBackend<'_> {
    fn terminated_text(&self, value: *const u32) -> RuntimeResult<Vec<u32>> {
        if value.is_null() { return Ok(vec![0]); }
        let mut result = Vec::new();
        for offset in 0..65_536usize {
            // SAFETY: callers pass live NUL-terminated Core TCHAR storage.
            let unit = unsafe { value.add(offset).read_unaligned() };
            result.push(unit);
            if unit == 0 { return Ok(result); }
        }
        Err(RuntimeError::new("SpawnPlayActor text exceeds the Core input limit"))
    }

    fn fstring_text(&self, value: *const TArrayRaw) -> RuntimeResult<Vec<u32>> {
        if value.is_null() { return Ok(vec![0]); }
        // SAFETY: value is a live FString field in a canonical object.
        let raw = unsafe { value.read_unaligned() };
        if raw.num < 0 || raw.max < raw.num || (raw.num > 0 && raw.data.is_null()) {
            return Err(RuntimeError::new("Invalid SpawnPlayActor FString storage"));
        }
        if raw.num == 0 { return Ok(vec![0]); }
        // SAFETY: validated FString count and backing pointer.
        let units = unsafe { std::slice::from_raw_parts(raw.data.cast::<u32>(), raw.num as usize) };
        Ok(units.to_vec())
    }

    fn text_eq(a: &[u32], b: &[u32]) -> bool {
        let mut left=a.iter().copied().take_while(|unit|*unit!=0);
        let mut right=b.iter().copied().take_while(|unit|*unit!=0);
        loop {
            match (left.next(),right.next()) {
                (None,None)=>return true,
                (Some(a),Some(b)) if char::from_u32(a).unwrap_or('\0').eq_ignore_ascii_case(&char::from_u32(b).unwrap_or('\0'))=>{},
                _=>return false,
            }
        }
    }

    fn option(&self, key: &str) -> RuntimeResult<Vec<u32>> {
        let key: Vec<u32> = key.encode_utf16().map(u32::from).collect();
        let url = unsafe { &*self.url };
        let options = url.op;
        if options.num < 0 || options.max < options.num || (options.num > 0 && options.data.is_null()) {
            return Err(RuntimeError::new("Invalid FURL option storage"));
        }
        for index in 0..options.num as usize {
            let raw = unsafe { options.data.cast::<TArrayRaw>().add(index).read_unaligned() };
            let option = self.fstring_text(&raw)?;
            let text=option.iter().copied().take_while(|unit|*unit!=0).collect::<Vec<_>>();
            if text.len()<key.len() || !text[..key.len()].iter().zip(&key).all(|(a,b)|a.eq_ignore_ascii_case(b)) { continue; }
            if key.last()==Some(&u32::from(b'=')) { let mut value=text[key.len()..].to_vec(); value.push(0); return Ok(value); }
            if text.len()==key.len() { return Ok(vec![0]); }
            if text[key.len()]==u32::from(b'=') { let mut value=text[key.len()+1..].to_vec(); value.push(0); return Ok(value); }
        }
        Ok(vec![0])
    }
    fn has_option(&self, key:&str) -> RuntimeResult<bool> {
        let key=key.encode_utf16().map(u32::from).collect::<Vec<_>>();
        let options=unsafe{(*self.url).op};
        if options.num<0 || options.max<options.num || (options.num>0&&options.data.is_null()) {
            return Err(RuntimeError::new("Invalid FURL option storage"));
        }
        for index in 0..options.num as usize {
            let raw=unsafe{options.data.cast::<TArrayRaw>().add(index).read_unaligned()};
            let text=self.fstring_text(&raw)?;
            let text=text.iter().copied().take_while(|unit|*unit!=0).collect::<Vec<_>>();
            if text.len()>=key.len() && text[..key.len()].iter().zip(&key).all(|(a,b)|a.eq_ignore_ascii_case(b))
                && (text.len()==key.len() || text[key.len()]==u32::from(b'='))
            { return Ok(true); }
        }
        Ok(false)
    }
    fn option_at(&self, index:usize) -> RuntimeResult<Vec<u32>> {
        let options=unsafe{(*self.url).op};
        if options.num<0 || options.max<options.num || (options.num>0&&options.data.is_null()) || index>=options.num as usize {
            return Err(RuntimeError::new("Invalid FURL option index or storage"));
        }
        let raw=unsafe{options.data.cast::<TArrayRaw>().add(index).read_unaligned()};
        self.fstring_text(&raw)
    }
    fn player_name_option(&self) -> RuntimeResult<usize> {
        let defaults=crate::url_owner::UrlOwner::current_defaults(self.access)?;
        const NAME: [u32; 6] = [78, 65, 77, 69, 61, 0];
        let fallback=defaults.default_name();
        let value=unsafe{crate::url_owner::UrlOwner::get_option(self.access,&defaults,self.url,NAME.as_ptr(),fallback.as_ptr())?};
        let text=self.terminated_text(value)?;
        let mut scratch=self.scratch.borrow_mut();
        scratch.player_name=text;
        Ok(scratch.player_name.as_ptr() as usize)
    }
    fn option_pointer(&self, key: &str, which: u8) -> RuntimeResult<usize> {
        let option_key=key.strip_suffix('=').unwrap_or(key);
        let present=self.has_option(option_key)?;
        let value=if present{self.option(key)?}else{vec![0]};
        let mut scratch = self.scratch.borrow_mut();
        let target = match which {
            0 => &mut scratch.class_option,
            1 => &mut scratch.load_option,
            _ => &mut scratch.player_name,
        };
        *target = value;
        Ok(if present { target.as_ptr() as usize } else { 0 })
    }

    fn level_info(&self) -> RuntimeResult<*mut ALevelInfo> {
        unsafe { crate::package_map_level_owner::level_info(self.level) }
    }

    fn event(&self, object: *mut UObject, name: &str) -> RuntimeResult<()> {
        if object.is_null() { return Err(RuntimeError::new("SpawnPlayActor event receiver is null")); }
        let name=self.access.intern_str(name,FindName::Find)?;
        let class=self.access.native_class("Function")?;
        let function=unsafe{self.access.find_field((*object).class.cast(),name,class)}.cast::<hp2_abi::engine::generated::UFunction>();
        if function.is_null(){return Err(RuntimeError::new("SpawnPlayActor reflected event is missing"));}
        if unsafe{(*function).parms_size}!=0 || unsafe{(*function).num_parms}!=0 {
            return Err(RuntimeError::new("SpawnPlayActor zero-parameter event metadata is invalid"));
        }
        unsafe { crate::actor_owner::process_event(self.access,object.cast::<AActor>(),function,ptr::null_mut(),ptr::null_mut())?; }
        Ok(())
    }

    fn error_text(&self, text: &[u32]) -> RuntimeResult<()> {
        if self.error.is_null() { return Err(RuntimeError::new("SpawnPlayActor error string is null")); }
        if text.last()==Some(&0) {
            unsafe { self.access.assign_fstring(self.error,text)?; }
        } else {
            let mut terminated=text.to_vec();
            terminated.push(0);
            unsafe { self.access.assign_fstring(self.error,&terminated)?; }
        }
        Ok(())
    }

    fn load_class(&self, name: &[u32], base: *mut UClass, flags: u32) -> RuntimeResult<*mut UClass> {
        let name = name.iter().copied().take_while(|unit| *unit != 0).collect::<Vec<_>>();
        unsafe { self.access.static_load_class_with_package_map(base, ptr::null_mut(), &name, flags, self.package_map.get()) }
    }

}


impl Backend for CoreSpawnPlayBackend<'_> {
    type Error = RuntimeError;
    fn invalid_entry(&self) -> Self::Error { RuntimeError::new("Invalid SpawnPlayActor continuation entry") }

    fn call(&self, op: Op, a: usize, b: usize) -> RuntimeResult<usize> {
        let level = self.level;
        let a_actor = a as *mut AActor;
        let result = match op {
            Op::ClearError => { self.error_text(&[0])?; 0 }
            Op::PackageMap => {
                let connection = a as *mut UNetConnection;
                self.package_map.set(unsafe { (*connection).package_map }.cast());
                0
            }
            Op::ClassOption => self.option_pointer("CLASS=", 0)?,
            Op::LoadPlayer => self.load_class(a as *const u32, self.access.native_class("Engine.PlayerPawn")?, LOAD_NO_WARN)? as usize,
            Op::DefaultUser => DEFAULT_USER.as_ptr() as usize,
            Op::DefaultIni => DEFAULT_INI.as_ptr() as usize,
            Op::ClassError => {
                let text=self.access.localize_text(hp2_core::lit!("Errors"),hp2_core::lit!("LoadPlayerClass"),Some(hp2_core::lit!("Engine")),None,false)?;
                self.error_text(&text)?;
                return Err(RuntimeError::from_utf32(text.into_iter().take_while(|unit|*unit!=0).collect()));
            }
            Op::SaveClass => { self.player_class.set(a as *mut UClass); 0 }
            Op::OptionCount => unsafe { (*self.url).op.num.max(0) as usize },
            Op::Question => { let mut s=self.scratch.borrow_mut(); if s.options.is_empty(){s.options.push(u32::from(b'?'));} else {s.options.insert(s.options.len()-1,u32::from(b'?'));} 0 }
            Op::Option => { let option=self.option_at(a)?; let mut s=self.scratch.borrow_mut(); s.options.pop(); s.options.extend(option.into_iter().take_while(|c|*c!=0)); s.options.push(0); 0 }
            Op::Login => { let game=unsafe{(*self.level_info()?).game}; let mut actor=ptr::null_mut(); unsafe { self.login_event(game.cast(), &mut actor)?; } self.actor.set(actor); actor as usize }
            Op::LoginFailed => { let mut message="Login failed: ".encode_utf16().map(u32::from).collect::<Vec<_>>(); message.extend(self.fstring_text(self.error)?.iter().copied().take_while(|unit|*unit!=0)); self.log("Warning",&message)?; 0 }
            Op::HasLoad => usize::from(self.has_option("load")?),
            Op::LoadValue => self.option_pointer("load=", 1)?,
            Op::SaveLoading => { self.scratch.borrow_mut().loading=a!=0; 0 }
            Op::ClearTravel => { self.clear_travel_info()?; 0 }
            Op::SetPlayer => { unsafe { (*self.actor.get()).player=self.player.cast(); } 0 }
            Op::SetRole => { unsafe { (*self.actor.get()).base.role=ROLE_AUTHORITY; } 0 }
            Op::SetRemoteRole => { unsafe { (*self.actor.get()).base.remote_role=self.role as u8; } 0 }
            Op::SetShow => { unsafe { (*self.actor.get()).show_flags=(SHOW_BACKDROP|SHOW_ACTORS|SHOW_PLAYER_CTRL|SHOW_REAL_TIME) as i32; } 0 }
            Op::SetRender => { unsafe { (*self.actor.get()).rend_map=REND_DYN_LIGHT; } 0 }
            Op::AllAdmin => {
                const ALL_ADMIN: [u32; 9] = [97, 108, 108, 97, 100, 109, 105, 110, 0];
                usize::from(unsafe{hp2_core::search::parse_param(self.access.command_line().as_ptr().cast(),ALL_ADMIN.as_ptr().cast())})
            }
            Op::NetDriver => usize::from(!unsafe{(*level).net_driver}.is_null()),
            Op::SetAdmin => { unsafe { (*self.actor.get()).b_admin=1; } 0 }
            Op::PreAccept => { self.event(a_actor.cast(), "TravelPreAccept")?; 0 }
            Op::FindTravel => self.find_travel(a as *const u32)?,
            Op::PlayerName => self.player_name_option()?,
            Op::TravelText => self.travel_value(a as *const u32)?,
            Op::Standalone => usize::from(unsafe{(*self.level_info()?).net_mode}==0),
            Op::SaveStr => { let mut scratch=self.scratch.borrow_mut(); scratch.input=self.terminated_text(a as *const u32)?; scratch.input_cursor=0; scratch.line.clear(); 0 }
            Op::Str => { let scratch=self.scratch.borrow(); unsafe{scratch.input.as_ptr().add(scratch.input_cursor)} as usize }
            Op::ParseClass => self.parse_token("CLASS=", true)? as usize,
            Op::ParseName => self.parse_token("NAME=", false)? as usize,
            Op::Incoming => { let mut msg="Incoming travelling actor of class ".encode_utf16().map(u32::from).collect::<Vec<_>>(); msg.extend(self.scratch.borrow().class_name.iter().copied().take_while(|unit|*unit!=0)); self.log("Log",&msg)?; 0 }
            Op::LoadTravel => self.load_class(self.scratch.borrow().class_name.as_slice(), self.access.native_class("Engine.Actor")?, LOAD_NO_WARN|LOAD_ALLOW_DLL)? as usize,
            Op::CannotAccept | Op::SpawnFailed => {
                let prefix=if matches!(op,Op::CannotAccept){"SpawnPlayActor: Cannot accept travelling class '"}else{"SpawnPlayActor: Failed to spawn travelling class '"};
                let mut message=prefix.encode_utf16().map(u32::from).collect::<Vec<_>>();
                message.extend(self.scratch.borrow().class_name.iter().copied().take_while(|unit|*unit!=0));
                message.push(u32::from(b'\''));
                self.log("Log",&message)?;
                0
            }
            Op::Spawned => { let msg="SpawnPlayActor: Spawned travelling actor".encode_utf16().map(u32::from).collect::<Vec<_>>(); self.log("Log",&msg)?; 0 }
            Op::Accept => { let actor=a_actor; let name=self.scratch.borrow().actor_name.clone(); let mut scratch=self.scratch.borrow_mut(); scratch.accepted.push(AcceptedActor{actor,name,parameters:Vec::new()}); scratch.accepted.len() }
            Op::ParseLine => self.parse_line()? as usize,
            Op::CompareEndLine => usize::from(self.scratch.borrow().line.as_slice()!=[u32::from(b'}'),0]),
            Op::SaveLine => { let line=self.scratch.borrow().line.clone(); if let Some(accepted)=self.scratch.borrow_mut().accepted.get_mut(a.saturating_sub(1)){accepted.parameters.push(line);} 0 }
            Op::AcceptedCount => self.scratch.borrow().accepted.len(),
            Op::ParmCount => self.scratch.borrow().accepted.get(a).map_or(0,|x|x.parameters.len()),
            Op::Parm => self.scratch.borrow().accepted.get(a).and_then(|x|x.parameters.get(b)).map_or(0,|x|x.as_ptr() as usize),
            Op::Char => unsafe { if a==0 {0} else {*(a as *const u32) as usize} },
            Op::Advance => unsafe { (a as *const u32).add(b) as usize },
            Op::Alnum => usize::from(char::from_u32(a as u32).is_some_and(char::is_alphanumeric)),
            Op::Digit => usize::from(char::from_u32(a as u32).is_some_and(|c| c.is_ascii_digit())),
            Op::VarChar => { let mut s=self.scratch.borrow_mut(); if a>=CLASS_BUFFER{return Err(RuntimeError::new("Travel property name exceeds limit"));} if s.variable.len()<=a{s.variable.resize(a+1,0);} s.variable[a]=b as u32; 0 }
            Op::Atoi => self.atoi(a as *const u32)? as usize,
            Op::SaveValue => { let mut s=self.scratch.borrow_mut(); s.value=self.terminated_text(a as *const u32)?; s.element=b as i32; 0 }
            Op::IteratorValid => usize::from(!self.scratch.borrow().iterator.is_null()),
            Op::IteratorNext => { self.next_property()?; 0 }
            Op::PropertyFlags => {
                let field=self.scratch.borrow().iterator;
                self.scratch.borrow_mut().property=field.cast::<UProperty>();
                unsafe{(*field.cast::<UProperty>()).property_flags as usize}
            }
            Op::ComparePropertyName => {
                let field=self.scratch.borrow().iterator.cast::<UObject>();
                let name=unsafe{self.access.name_text(i32::from_ne_bytes((*field).name))?};
                let variable=self.scratch.borrow().variable.clone();
                let actual=name.iter().map(|unit|*unit as u32).collect::<Vec<_>>();
                usize::from(!Self::text_eq(&actual,&variable))
            }
            Op::ArrayDim => unsafe{(*self.scratch.borrow().property).array_dim as usize},
            Op::Data => {
                let accepted=self.scratch.borrow().accepted.get(a).ok_or_else(||RuntimeError::new("Invalid accepted Actor index"))?.actor;
                let property=self.scratch.borrow().property;
                let element=self.scratch.borrow().element;
                if element<0{return Ok(0);}
                let offset=unsafe{(*property).offset.checked_add(element.checked_mul((*property).element_size).ok_or_else(||RuntimeError::new("Travel property element offset overflow"))?).ok_or_else(||RuntimeError::new("Travel property offset overflow"))?};
                let data=unsafe{accepted.cast::<u8>().add(offset as usize)};
                self.scratch.borrow_mut().data=data;
                data as usize
            }
            Op::ObjectProperty => {
                let cls=self.access.native_class("ObjectProperty")?;
                usize::from(unsafe{hp2_core::object_model::is_a(self.scratch.borrow().property.cast(),cls.cast())})
            }
            Op::ActorReference => {
                let property=a as *mut hp2_abi::engine::generated::UObjectProperty;
                let actor_class=self.access.native_class("Engine.Actor")?;
                usize::from(unsafe{hp2_core::object_model::is_child_of((*property).property_class.cast(),actor_class.cast())})
            }
            Op::NameMatches => {
                let name=self.scratch.borrow().accepted.get(a).map(|x|x.name.clone()).unwrap_or_default();
                usize::from(Self::text_eq(&name,&self.scratch.borrow().value))
            }
            Op::StoreReference => { let actor=self.scratch.borrow().accepted.get(a).ok_or_else(||RuntimeError::new("Invalid accepted Actor index"))?.actor; unsafe{self.scratch.borrow().data.cast::<*mut UObject>().write_unaligned(actor.cast())}; 0 }
            Op::Import => {
                let property=self.scratch.borrow().property;
                let data=self.scratch.borrow().data;
                let value=self.scratch.borrow().value.clone();
                unsafe{self.access.property_import_text_at(property,data,&value,ptr::null_mut(),0)?}; 0
            }
            Op::AcceptedActor => self.scratch.borrow().accepted.get(a).map_or(0,|x|x.actor as usize),
            Op::AcceptInventory => { let game=unsafe{(*self.level_info()?).game}; self.event_actor(game.cast(),"AcceptInventory",self.actor.get().cast())?; 0 }
            Op::PostAccept => { self.event(a_actor.cast(), "TravelPostAccept")?; 0 }
            Op::PreviousProperty => {
                let class=unsafe{(*self.actor.get()).base.base.class.cast::<UClass>()};
                let name=self.access.intern_str("PreviousLevelName",FindName::Find)?;
                self.access.find_field(class.cast(),name,self.access.native_class("StrProperty")?) as usize
            }
            Op::PreviousFlags => unsafe{(*((a as *mut UProperty))).property_flags as usize},
            Op::ClearPrevious => {
                let actor=self.actor.get().cast::<UObject>();
                let property=a as *mut UProperty;
                let address=unsafe{self.access.property_address(actor,property)?};
                let text=[0u32];
                unsafe{self.access.assign_fstring(address.cast(),&text)?}; 0
            }
            Op::Actor => self.actor.get() as usize,
            Op::Loading => usize::from(self.scratch.borrow().loading),
            Op::FirstValid => self.first_travel_pair()? as usize,
            Op::FirstValue => self.scratch.borrow().travel_text.as_ptr() as usize,
            Op::PostLogin => { let game=unsafe{(*self.level_info()?).game}; self.event_actor(game.cast(),"PostLogin",self.actor.get().cast())?; 0 }
            Op::Element => self.scratch.borrow().element as usize,
            Op::Connection => if unsafe{hp2_core::object_model::is_a(self.player,self.access.native_class("Engine.NetConnection")?.cast())} {self.player as usize} else {0},
        };
        Ok(result)
    }

    fn scope(&self, entry: u32, arg: usize) -> RuntimeResult<usize> {
        match entry {
            1 => { let mut s=self.scratch.borrow_mut(); s.options.clear(); s.options.push(0); }
            2 => { let mut s=self.scratch.borrow_mut(); s.accepted.clear(); }
            3 => {
                let actor=self.scratch.borrow().accepted.get(arg).ok_or_else(||RuntimeError::new("Invalid travelling actor scope"))?.actor;
                let class=unsafe{(*actor.cast::<UObject>()).class.cast::<hp2_abi::engine::generated::UStruct>()};
                if class.is_null(){return Err(RuntimeError::new("Travelling Actor has no reflected class"));}
                let mut scratch=self.scratch.borrow_mut();
                scratch.actor_index=arg;
                scratch.iterator_struct=class;
                scratch.iterator=unsafe{(*class).children};
                drop(scratch);
                self.next_property_from_start()?;
            }
            4 => {}
            _ => return Err(self.invalid_entry()),
        }
        level_spawn_play_actor::run_with(self,entry,arg)
    }
}

impl CoreSpawnPlayBackend<'_> {
    fn find_travel(&self, name: *const u32) -> RuntimeResult<usize> {
        let key=self.terminated_text(name)?;
        let map=unsafe{ptr::addr_of!((*self.level).travel_info).read_unaligned()};
        let pairs=map.pairs;
        if pairs.num<0 || pairs.max<pairs.num || (pairs.num>0&&pairs.data.is_null()) {
            return Err(RuntimeError::new("Invalid Level TravelInfo storage"));
        }
        for index in 0..pairs.num as usize {
            let pair=unsafe{pairs.data.cast::<hp2_abi::engine::generated::FStringMapPair>().add(index)};
            let key_units=unsafe{self.fstring_text(ptr::addr_of!((*pair).key))?};
            if Self::text_eq(&key_units,&key) { return Ok(pair as usize); }
        }
        Ok(0)
    }
    fn travel_value(&self, pair: *const u32) -> RuntimeResult<usize> {
        let pair=pair.cast::<hp2_abi::engine::generated::FStringMapPair>();
        if pair.is_null() { return Ok(0); }
        let value=unsafe{self.fstring_text(ptr::addr_of!((*pair).value))?};
        let mut scratch=self.scratch.borrow_mut();
        scratch.travel_text=value;
        Ok(scratch.travel_text.as_ptr() as usize)
    }
    fn parse_token(&self, token: &str, class: bool) -> RuntimeResult<bool> {
        let input=self.scratch.borrow().input.clone();
        let token:Vec<u32>=token.encode_utf16().map(u32::from).collect();
        let mut i=0; while i+token.len()<=input.len() && !input[i..i+token.len()].iter().zip(&token).all(|(a,b)|a.eq_ignore_ascii_case(b)){i+=1;}
        if i+token.len()>input.len(){return Ok(false);}
        let mut j=i+token.len(); while j<input.len()&&input[j]!=0&&input[j]!=u32::from(b' '){j+=1;}
        let mut value=input[i+token.len()..j].to_vec(); value.push(0);
        let mut s=self.scratch.borrow_mut(); if class{s.class_name=value;}else{s.actor_name=value;} Ok(true)
    }
    fn parse_line(&self) -> RuntimeResult<bool> {
        let mut scratch=self.scratch.borrow_mut();
        let current=unsafe{scratch.input.as_ptr().add(scratch.input_cursor)};
        let mut cursor=current;
        let mut output=[0u32;256];
        let has_more=unsafe{hp2_core::tokenize::parse_line(&mut cursor,output.as_mut_ptr(),256,true)};
        let next=unsafe{cursor.offset_from(scratch.input.as_ptr())};
        if next<0 || next as usize>scratch.input.len(){return Err(RuntimeError::new("Core ParseLine advanced outside travel input"));}
        let length=output.iter().position(|unit|*unit==0).unwrap_or(output.len()-1);
        scratch.line.clear();
        scratch.line.extend_from_slice(&output[..length]);
        scratch.line.push(0);
        scratch.input_cursor=next as usize;
        Ok(has_more)
    }
    fn atoi(&self, pointer:*const u32)->RuntimeResult<i32>{
        if pointer.is_null(){return Ok(0);}
        let _text=self.terminated_text(pointer)?;
        match unsafe{hp2_core::ansi::atoi_checked(pointer)} {
            Ok(value)=>Ok(value),
            Err(value)=>Err(RuntimeError::new(&format!("appAtoi integer overflow: {value}"))),
        }
    }
    fn event_actor(&self, object:*mut UObject, name:&str, actor:*mut AActor)->RuntimeResult<()> {
        if object.is_null(){return Err(RuntimeError::new("SpawnPlayActor event receiver is null"));}
        let name_index=self.access.intern_str(name,FindName::Find)?;
        let function_class=self.access.native_class("Function")?;
        let function=unsafe{self.access.find_field((*object).class.cast(),name_index,function_class)}.cast::<hp2_abi::engine::generated::UFunction>();
        if function.is_null(){return Err(RuntimeError::new("SpawnPlayActor reflected event is missing"));}
        if unsafe{(*function).parms_size as usize}!=std::mem::size_of::<*mut AActor>() || unsafe{(*function).num_parms}!=1 {
            return Err(RuntimeError::new("SpawnPlayActor event parameter layout does not match metadata"));
        }
        let mut parameter=actor;
        unsafe{crate::actor_owner::process_event(self.access,object.cast::<AActor>(),function,(&mut parameter as *mut *mut AActor).cast::<c_void>(),ptr::null_mut())?;}
        Ok(())
    }
    unsafe fn login_event(&self, game:*mut UObject, actor:&mut *mut APlayerPawn)->RuntimeResult<()> {
        let name=self.access.intern_str("Login",FindName::Find)?;
        let function_class=self.access.native_class("Function")?;
        let function=unsafe{self.access.find_field((*game).class.cast(),name,function_class)}.cast::<hp2_abi::engine::generated::UFunction>();
        if function.is_null(){return Err(RuntimeError::new("Game.Login event is missing"));}
        if unsafe{(*function).parms_size as usize}!=std::mem::size_of::<AGameInfoLoginParams>()
            || unsafe{(*function).num_parms}!=5
            || unsafe{(*function).return_value_offset as usize}!=std::mem::offset_of!(AGameInfoLoginParams,return_value)
        {
            return Err(RuntimeError::new("Game.Login parameter layout does not match Core metadata"));
        }
        let mut params: AGameInfoLoginParams=unsafe{std::mem::zeroed()};
        let portal=unsafe{self.fstring_text(ptr::addr_of!((*self.url).portal))?};
        let options=self.scratch.borrow().options.clone();
        let error=unsafe{self.fstring_text(self.error)?};
        let execution=(|| {
            unsafe {
                self.access.assign_fstring(ptr::addr_of_mut!(params.portal),&portal)?;
                self.access.assign_fstring(ptr::addr_of_mut!(params.options),&options)?;
                self.access.assign_fstring(ptr::addr_of_mut!(params.error),&error)?;
            }
            params.spawn_class=self.player_class.get().cast();
            unsafe{crate::actor_owner::process_event(self.access,game.cast::<AActor>(),function,(&mut params as *mut AGameInfoLoginParams).cast::<c_void>(),ptr::null_mut())?;}
            let output=unsafe{self.fstring_text(ptr::addr_of!(params.error))?};
            unsafe{self.access.assign_fstring(self.error,&output)?;}
            *actor=params.return_value.cast();
            Ok(())
        })();
        let mut cleanup_error=None;
        for field in [
            ptr::addr_of_mut!(params.portal),
            ptr::addr_of_mut!(params.options),
            ptr::addr_of_mut!(params.error),
        ] {
            if let Err(error)=unsafe{self.access.destroy_owned_fstring(field)} {
                if cleanup_error.is_none(){cleanup_error=Some(error);}
            }
        }
        execution?;
        cleanup_error.map_or(Ok(()),Err)
    }
    fn next_property_from_start(&self) -> RuntimeResult<()> {
        let candidate=self.scratch.borrow().iterator;
        self.find_property(candidate)
    }
    fn next_property(&self) -> RuntimeResult<()> {
        let current=self.scratch.borrow().iterator;
        let candidate=if current.is_null(){ptr::null_mut()}else{unsafe{(*current.cast::<hp2_abi::engine::generated::UField>()).next}};
        self.find_property(candidate)
    }
    fn find_property(&self, mut candidate:*mut c_void) -> RuntimeResult<()> {
        loop {
            if candidate.is_null() {
                let mut scratch=self.scratch.borrow_mut();
                if scratch.iterator_struct.is_null() { scratch.iterator=ptr::null_mut(); return Ok(()); }
                scratch.iterator_struct=unsafe{(*scratch.iterator_struct.cast::<hp2_abi::engine::generated::UField>()).super_field.cast::<hp2_abi::engine::generated::UStruct>()};
                candidate=if scratch.iterator_struct.is_null(){ptr::null_mut()}else{unsafe{(*scratch.iterator_struct).children}};
                if candidate.is_null(){continue;}
            }
            let property_class=self.access.native_class("Property")?;
            if unsafe{hp2_core::object_model::is_child_of((*candidate.cast::<UObject>()).class,property_class.cast())} {
                self.scratch.borrow_mut().iterator=candidate;
                return Ok(());
            }
            candidate=unsafe{(*candidate.cast::<hp2_abi::engine::generated::UField>()).next};
        }
    }
    fn first_travel_pair(&self) -> RuntimeResult<bool> {
        let map=unsafe{ptr::addr_of!((*self.level).travel_info).read_unaligned()};
        let pairs=map.pairs;
        if pairs.num<0 || pairs.max<pairs.num || (pairs.num>0&&pairs.data.is_null())
            || map.hash_count<0 || (map.hash_count>0&&map.hash.is_null())
        { return Err(RuntimeError::new("Invalid Level TravelInfo map storage")); }
        if pairs.num == 0 { return Ok(false); }
        let pair=unsafe{pairs.data.cast::<hp2_abi::engine::generated::FStringMapPair>()};
        self.scratch.borrow_mut().travel_text=unsafe{self.fstring_text(ptr::addr_of!((*pair).value))?};
        Ok(true)
    }

    fn clear_travel_info(&self) -> RuntimeResult<()> {
        let map=ptr::addr_of_mut!(unsafe{(*self.level).travel_info});
        let pairs=unsafe{(*map).pairs};
        if pairs.num<0 || pairs.max<pairs.num || (pairs.num>0&&pairs.data.is_null()) {
            return Err(RuntimeError::new("Invalid Level TravelInfo pairs"));
        }
        let mut first=None;
        for index in 0..pairs.num as usize {
            let pair=unsafe{pairs.data.cast::<hp2_abi::engine::generated::FStringMapPair>().add(index)};
            for field in [ptr::addr_of_mut!(unsafe{(*pair).key}),ptr::addr_of_mut!(unsafe{(*pair).value})] {
                if let Err(error)=unsafe{self.access.destroy_owned_fstring(field)} { if first.is_none(){first=Some(error);} }
            }
        }
        let allocator=hp2_core::array_storage::system_allocator();
        let slots=hp2_core::array_storage::Hp2ArraySlots {
            data:unsafe{ptr::addr_of_mut!((*map).pairs.data)},
            num:unsafe{ptr::addr_of_mut!((*map).pairs.num)},
            max:unsafe{ptr::addr_of_mut!((*map).pairs.max)},
        };
        if let Err(code)=unsafe{hp2_core::array_storage::release(slots,allocator)} {
            if first.is_none(){first=Some(RuntimeError::new(&format!("Unable to release Level TravelInfo pairs ({code})")));}
        }
        let old_hash=unsafe{(*map).hash};
        if !old_hash.is_null() && unsafe{(allocator.free)(allocator.context,old_hash)}!=0 && first.is_none() {
            first=Some(RuntimeError::new("Unable to release Level TravelInfo hash"));
        }
        let mut hash=ptr::null_mut();
        if unsafe{(allocator.realloc)(allocator.context,ptr::null_mut(),8*std::mem::size_of::<i32>(),ptr::null(),std::mem::size_of::<i32>() as i32,ptr::null(),&mut hash)}!=0 {
            if first.is_none(){first=Some(RuntimeError::new("Unable to initialize Level TravelInfo hash"));}
        } else {
            unsafe{std::slice::from_raw_parts_mut(hash.cast::<i32>(),8).fill(-1);}
            unsafe{(*map).hash=hash;(*map).hash_count=8;}
        }
        first.map_or(Ok(()),Err)
    }
}

/// Spawns a player through the Core-owned world and executes login/travel policy in the Core VM.
///
/// # Errors
/// Returns Core ownership, reflection, class-loading, VM, allocation, or actor-spawn errors.
///
/// # Safety
/// All input records and their reachable Core storage must be live for the synchronous call.
pub unsafe fn spawn_play_actor(
    access: CoreAccess<'_>,
    level: *mut ULevel,
    connection: *mut UNetConnection,
    role: i32,
    url: *mut FURL,
    error: *mut TArrayRaw,
) -> RuntimeResult<*mut AActor> {
    if level.is_null() || url.is_null() || error.is_null() {
        return Err(RuntimeError::new("Null SpawnPlayActor input"));
    }
    let backend=CoreSpawnPlayBackend { access, level, player: connection.cast(), role, url, error, package_map:std::cell::Cell::new(ptr::null_mut()), player_class:std::cell::Cell::new(ptr::null_mut()), actor:std::cell::Cell::new(ptr::null_mut()), scratch:RefCell::new(Scratch::default()) };
    let result=level_spawn_play_actor::run_with(&backend,0,0)?;
    Ok(result as *mut AActor)
}
