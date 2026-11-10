include!("console_ini_retail_restart.rs");

struct CountingTransactions(std::rc::Rc<std::cell::Cell<u32>>);
impl hp2_core::core_access::ObjectTransactions for CountingTransactions {
    unsafe fn save_object(&self, _: CoreAccess<'_>, _: *mut UObject) -> RuntimeResult<()> { self.0.set(self.0.get() + 1); Ok(()) }
    unsafe fn save_object_array_range(&self, _: CoreAccess<'_>, _: *mut UObject,
        _: *mut hp2_abi::engine::generated::TTransArrayRaw, _: i32, _: i32,
        _: hp2_core::core_access::ObjectArrayTransactionOperation, _: usize) -> RuntimeResult<()> {
        self.0.set(self.0.get() + 1); Ok(())
    }
}

#[test]
fn runtime_transactional_actor_accepts_stock_menu_command_without_manager() {
    let data = PathBuf::from(std::env::var_os("HP2_CORE_DATA_ROOT").expect("prepared retail data"));
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join(format!("console-retail-transactional-{}", std::process::id()));
    copy_system(&data.join("System"), &root.join("System"));
    for dir in ["Maps", "Textures", "Sounds", "Music"] {
        let source = data.join(dir);
        if source.is_dir() { std::os::unix::fs::symlink(std::fs::canonicalize(source).unwrap(), root.join(dir)).unwrap(); }
    }
    let (owner, pawn, class) = unsafe { boot_restart_world(&root) };
    let access = owner.access();
    // ULevel::SpawnActor marks spawned actors RF_Transactional in game mode.
    unsafe { (*pawn.cast::<UObject>()).object_flags |= 1; }
    unsafe { real_console(access, pawn.cast(), &string_arg("SET ini:HGame.Harry bAutoQuaff False")); }
    let value = unsafe { real_console(access, pawn.cast(), &string_arg("GET ini:HGame.Harry bAutoQuaff")) };
    assert!(value.eq_ignore_ascii_case("False"), "{value}");
    let name = access.intern_str("bAutoQuaff", FindName::Find).unwrap();
    let property = unsafe { access.find_field(class.cast(), name, access.native_class("Core.Property").unwrap()).cast::<UProperty>() };
    let address = unsafe { access.property_address(pawn.cast(), property) }.unwrap();
    let mask = unsafe { (*property.cast::<hp2_abi::engine::generated::UBoolProperty>()).bit_mask as u32 };
    assert_eq!(unsafe { address.cast::<u32>().read_unaligned() } & mask, 0, "live actor");
    let persisted = std::fs::read(root.join("System/DefUser.ini")).unwrap();
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    access.set_transactions(Some(std::rc::Rc::new(CountingTransactions(calls.clone()))));
    let error = unsafe { access.execute_static_command(wide("SET ini:HGame.Harry bAutoQuaff True").as_ptr(), &mut |_| Ok(())) }.unwrap_err();
    let diagnostic: String = error.diagnostic().iter().copied().filter_map(char::from_u32).collect();
    assert!(diagnostic.contains("Cannot safely notify this actor"), "{diagnostic}");
    assert_eq!(calls.get(), 0, "transaction must not be notified before a rejected publication");
    assert_eq!(std::fs::read(root.join("System/DefUser.ini")).unwrap(), persisted);
    let value = unsafe { real_console(access, pawn.cast(), &string_arg("GET ini:HGame.Harry bAutoQuaff")) };
    assert!(value.eq_ignore_ascii_case("False"), "default changed after failed transaction preflight: {value}");
    assert_eq!(unsafe { address.cast::<u32>().read_unaligned() } & mask, 0, "live actor changed after failed preflight");
    access.set_transactions(None);
    owner.shutdown().unwrap();
}
