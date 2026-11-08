use egui::Ui;
use hp2_core::core_access::RuntimeResult;
use hp2_launcher::model::{
    DataSource, LaunchAction, LaunchSelection, LauncherPaths, LauncherSettings, SaveRecord,
};
use hp2_launcher::orchestrate;

#[derive(Clone)]
pub struct DataSourceOption {
    pub source: DataSource,
    pub root: String,
    pub available: bool,
    pub error: Option<String>,
}
struct RetailImportFrame {
    state: *mut LauncherUiState,
    source: String,
    destination: String,
    root: String,
    canonical_root: String,
    root_error: String,
    progress: std::sync::Arc<hp2_import::output::Progress>,
}

#[derive(Default)]
struct ImportResult {
    error: String,
}

struct ImportWorkerFrame {
    source: String,
    destination: String,
    progress: std::sync::Arc<hp2_import::output::Progress>,
    result: std::sync::Arc<std::sync::Mutex<ImportResult>>,
    succeeded: std::sync::Arc<std::sync::atomic::AtomicBool>,
    done: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

unsafe extern "C" fn retail_import_call(
    raw: *mut std::ffi::c_void,
    leaf: hp2_launcher::import_session::Hp2LauncherImportSessionLeaf,
    index: usize,
    value: i32,
    out: *mut i64,
) -> i32 {
    use hp2_launcher::import_session::Hp2LauncherImportSessionLeaf as L;
    let frame = unsafe { &mut *raw.cast::<RetailImportFrame>() };
    let state = unsafe { &mut *frame.state };
    let mut answer = 0;
    match leaf {
        L::Importing => answer = i64::from(state.importing),
        L::SourceEmpty => answer = i64::from(frame.source.is_empty()),
        L::CaptureDestination => frame.destination.clone_from(&state.retail_data_root),
        L::DestinationEmpty => answer = i64::from(frame.destination.is_empty()),
        L::ConventionalDestination => {
            let result = hp2_bootstrap_paths::bootstrap_paths::run(5, b"", b"", &[], |_, _| 1);
            if result.success {
                let Some(path) = result.text.first() else {
                    return 1;
                };
                frame.destination = String::from_utf8_lossy(path).into_owned();
                answer = i64::from(!frame.destination.is_empty());
            }
        }
        L::PublishDestination => state.retail_data_root.clone_from(&frame.destination),
        L::ResetFilesDone => frame
            .progress
            .files_done
            .store(0, std::sync::atomic::Ordering::SeqCst),
        L::ResetFilesTotal => frame
            .progress
            .files_total
            .store(0, std::sync::atomic::Ordering::SeqCst),
        L::ResetCancellation => frame
            .progress
            .cancel_requested
            .store(false, std::sync::atomic::Ordering::Relaxed),
        L::ResetDone => state
            .import_done
            .store(false, std::sync::atomic::Ordering::SeqCst),
        L::SetImporting => {
            state.importing = value != 0;
            if !state.importing {
                state.import_progress = None;
            }
        }
        L::StartWorker => {
            state.import_progress = Some(frame.progress.clone());
            let worker = ImportWorkerFrame {
                source: frame.source.clone(),
                destination: frame.destination.clone(),
                progress: frame.progress.clone(),
                result: state.import_result.clone(),
                succeeded: state.import_succeeded.clone(),
                done: state.import_done.clone(),
            };
            state.import_task = Some(std::thread::spawn(move || {
                let services = hp2_launcher::import_session::Hp2LauncherImportSessionServices {
                    context: (&worker as *const ImportWorkerFrame).cast_mut().cast(),
                    call: retail_import_worker_call,
                    message: retail_import_worker_message,
                    pick_archive: retail_import_worker_pick_archive,
                    import_locals: retail_import_worker_locals,
                    publish_import_result: retail_import_worker_result,
                };
                if unsafe {
                    hp2_launcher::import_session::execute(
                        &services,
                        hp2_launcher::import_session::Hp2LauncherImportSessionAction::Worker,
                    )
                }
                .is_err()
                {
                    std::process::abort();
                }
            }));
        }
        L::CaptureRetailRoot => frame.root.clone_from(&state.retail_data_root),
        L::RootEmpty => answer = i64::from(frame.root.is_empty()),
        L::ValidateRetailRoot => match validate_root(&frame.root) {
            Ok(canonical) => {
                frame.canonical_root = canonical;
                answer = 1;
            }
            Err(error) => frame.root_error = error,
        },
        L::ValidatePrototypeRoot => match validate_root(&frame.source) {
            Ok(canonical) => {
                frame.canonical_root = canonical;
                answer = 1;
            }
            Err(error) => frame.root_error = error,
        },
        L::OptionCount => answer = state.data_source_options.len() as i64,
        L::OptionSource => {
            answer = state
                .data_source_options
                .get(index)
                .map_or(-1, |o| o.source as i64)
        }
        L::SetOptionRoot => {
            if let Some(option) = state.data_source_options.get_mut(index) {
                option.root = if value != 0 {
                    frame.canonical_root.clone()
                } else if option.source == DataSource::Prototype {
                    frame.source.clone()
                } else {
                    frame.root.clone()
                };
            }
        }
        L::SetOptionAvailable => {
            if let Some(option) = state.data_source_options.get_mut(index) {
                option.available = value != 0;
            }
        }
        L::ClearOptionError => {
            if let Some(option) = state.data_source_options.get_mut(index) {
                option.error = None;
            }
        }
        L::SetOptionError => {
            if let Some(option) = state.data_source_options.get_mut(index) {
                option.error = Some(frame.root_error.clone());
            }
        }
        L::SetRetailRoot => {
            state.retail_root = if value != 0 {
                frame.canonical_root.clone()
            } else {
                frame.root.clone()
            };
            if value != 0 && state.data_source == DataSource::Retail {
                state.needs_game_data = false;
            }
        }
        L::SetPrototypeRoot => {
            state.prototype_root = if value != 0 {
                frame.canonical_root.clone()
            } else {
                frame.source.clone()
            };
            if value != 0 && state.data_source == DataSource::Prototype {
                state.needs_game_data = false;
            }
        }
        L::SetSelectedSource => {
            state.data_source = if value == 0 {
                DataSource::Retail
            } else {
                DataSource::Prototype
            };
            state.needs_game_data = !state
                .data_source_options
                .iter()
                .find(|option| option.source == state.data_source)
                .is_some_and(|option| option.available);
        }
        L::CopyResultDataSources => {
            state.result_data_sources = Some(hp2_launcher::model::DataSourceConfiguration {
                selected: state.data_source,
                retail_root: state.retail_root.clone(),
                prototype_root: state.prototype_root.clone(),
            });
        }
        L::PickFolder => {
            if let Some(path) = hp2_platform::picker::pick_folder() {
                let Ok(path) = String::from_utf8(path) else {
                    return 1;
                };
                frame.source = path;
                answer = 1;
            }
        }
        L::MessageRootError => state.data_message = Some(frame.root_error.clone()),
        L::RootErrorEmpty => answer = i64::from(frame.root_error.is_empty()),
        L::ValidateSettings => {
            match hp2_launcher::store::validate_launcher_settings(&state.settings) {
                Ok(()) => answer = 1,
                Err(error) => {
                    state.error = Some(error);
                    answer = 0;
                }
            }
        }
        L::ClearValidationError => state.error = None,
        L::ResetSelection => state.action = Some(LaunchSelection::default()),
        L::SetSelectionAction => {
            if let Some(selection) = state.action.as_mut() {
                selection.action = match value {
                    0 => LaunchAction::Continue,
                    1 => LaunchAction::NewGame,
                    2 => LaunchAction::Quit,
                    _ => LaunchAction::Error,
                };
            }
        }
        L::SelectedSave => answer = state.selected_save as i64,
        L::SaveCountAsInt => answer = state.saves.len() as i64,
        L::SetSelectedSave => state.selected_save = index as i32,
        L::SaveCount => answer = state.saves.len() as i64,
        L::SaveIndex => {
            answer = state
                .saves
                .get(index)
                .map_or(-1, |save| save.save_index as i64)
        }
        L::CopyInitialSettings => state.result_settings = Some(state.settings.clone()),
        L::SetSelectionHasSave => {
            let Some(selection) = state.action.as_mut() else {
                return 1;
            };
            selection.has_save = true;
        }
        L::CopySelectedSave => {
            let Some(save) = usize::try_from(state.selected_save)
                .ok()
                .and_then(|index| state.saves.get(index))
                .cloned()
            else {
                return 1;
            };
            let Some(selection) = state.action.as_mut() else {
                return 1;
            };
            selection.save = save;
        }
        L::Joinable => answer = i64::from(state.import_task.is_some()),
        L::Join => {
            if let Some(task) = state.import_task.take() {
                if let Err(payload) = task.join() {
                    std::panic::resume_unwind(payload);
                }
            }
        }
        L::Succeeded => {
            answer = i64::from(
                state
                    .import_succeeded
                    .load(std::sync::atomic::Ordering::Relaxed),
            )
        }
        L::ImportErrorEmpty => {
            answer = i64::from(state.import_result.lock().unwrap().error.is_empty())
        }
        L::MessageImportError => {
            state.data_message = Some(state.import_result.lock().unwrap().error.clone());
        }
        L::SetFinished => state.finished = true,
        L::RequestCancellation => {
            if let Some(progress) = state.import_progress.as_ref() {
                progress
                    .cancel_requested
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        _ => return 1,
    }
    unsafe {
        out.write(answer);
    }
    0
}

unsafe extern "C" fn retail_import_worker_call(
    raw: *mut std::ffi::c_void,
    leaf: hp2_launcher::import_session::Hp2LauncherImportSessionLeaf,
    _: usize,
    value: i32,
    out: *mut i64,
) -> i32 {
    use hp2_launcher::import_session::Hp2LauncherImportSessionLeaf as L;
    let frame = unsafe { &*raw.cast::<ImportWorkerFrame>() };
    match leaf {
        L::SetSucceeded => frame
            .succeeded
            .store(value != 0, std::sync::atomic::Ordering::Relaxed),
        L::PublishDone => frame.done.store(true, std::sync::atomic::Ordering::Release),
        _ => return 1,
    }
    unsafe {
        out.write(0);
    }
    0
}

unsafe extern "C" fn retail_import_worker_message(
    raw: *mut std::ffi::c_void,
    text: *const u8,
    len: usize,
) -> i32 {
    let frame = unsafe { &*raw.cast::<ImportWorkerFrame>() };
    let bytes = unsafe { std::slice::from_raw_parts(text, len) };
    frame.result.lock().unwrap().error = String::from_utf8_lossy(bytes).into_owned();
    0
}

unsafe extern "C" fn retail_import_worker_pick_archive(
    _: *mut std::ffi::c_void,
    _: *const hp2_launcher::import_session::Hp2LauncherImportSessionExtension,
    _: usize,
    _: *mut bool,
) -> i32 {
    1
}

unsafe extern "C" fn retail_import_worker_locals(
    raw: *mut std::ffi::c_void,
    source_data: *mut *const u8,
    source_len: *mut usize,
    destination_data: *mut *const u8,
    destination_len: *mut usize,
    progress: *mut *mut std::ffi::c_void,
) -> i32 {
    let frame = unsafe { &*raw.cast::<ImportWorkerFrame>() };
    unsafe {
        source_data.write(frame.source.as_ptr());
        source_len.write(frame.source.len());
        destination_data.write(frame.destination.as_ptr());
        destination_len.write(frame.destination.len());
        progress.write(std::sync::Arc::as_ptr(&frame.progress).cast_mut().cast());
    }
    0
}

unsafe extern "C" fn retail_import_worker_result(
    raw: *mut std::ffi::c_void,
    _: *const u8,
    _: usize,
    error: *const u8,
    error_len: usize,
) -> i32 {
    let frame = unsafe { &*raw.cast::<ImportWorkerFrame>() };
    let error = unsafe { std::slice::from_raw_parts(error, error_len) };
    frame.result.lock().unwrap().error = String::from_utf8_lossy(error).into_owned();
    0
}

unsafe extern "C" fn retail_import_message(
    raw: *mut std::ffi::c_void,
    text: *const u8,
    len: usize,
) -> i32 {
    let frame = unsafe { &mut *raw.cast::<RetailImportFrame>() };
    let state = unsafe { &mut *frame.state };
    state.data_message = Some(
        String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(text, len) }).into_owned(),
    );
    0
}
unsafe extern "C" fn retail_import_pick_archive(
    raw: *mut std::ffi::c_void,
    spans: *const hp2_launcher::import_session::Hp2LauncherImportSessionExtension,
    count: usize,
    picked: *mut bool,
) -> i32 {
    let frame = unsafe { &mut *raw.cast::<RetailImportFrame>() };
    let spans = unsafe { std::slice::from_raw_parts(spans, count) };
    let mut owned = Vec::with_capacity(count);
    for span in spans {
        let bytes = unsafe { std::slice::from_raw_parts(span.data, span.len) };
        let Ok(value) = std::str::from_utf8(bytes) else {
            return 1;
        };
        owned.push(value.to_owned());
    }
    let extensions = owned.iter().map(String::as_str).collect::<Vec<_>>();
    if let Some(path) = hp2_platform::picker::pick_file(&extensions) {
        let Ok(path) = String::from_utf8(path) else {
            return 1;
        };
        frame.source = path;
        unsafe {
            picked.write(true);
        }
    } else {
        unsafe {
            picked.write(false);
        }
    }
    0
}
unsafe extern "C" fn retail_import_locals(
    raw: *mut std::ffi::c_void,
    source_data: *mut *const u8,
    source_len: *mut usize,
    destination_data: *mut *const u8,
    destination_len: *mut usize,
    progress: *mut *mut std::ffi::c_void,
) -> i32 {
    let frame = unsafe { &mut *raw.cast::<RetailImportFrame>() };
    unsafe {
        source_data.write(frame.source.as_ptr());
        source_len.write(frame.source.len());
        destination_data.write(frame.destination.as_ptr());
        destination_len.write(frame.destination.len());
        progress.write(std::sync::Arc::as_ptr(&frame.progress).cast_mut().cast());
    }
    0
}
unsafe extern "C" fn retail_import_result(
    raw: *mut std::ffi::c_void,
    _: *const u8,
    _: usize,
    error: *const u8,
    error_len: usize,
) -> i32 {
    let frame = unsafe { &mut *raw.cast::<RetailImportFrame>() };
    let state = unsafe { &mut *frame.state };
    let error = unsafe { std::slice::from_raw_parts(error, error_len) };
    state.import_result.lock().unwrap().error = String::from_utf8_lossy(error).into_owned();
    0
}
pub struct LauncherUiState {
    pub data_source_options: Vec<DataSourceOption>,
    pub settings: LauncherSettings,
    pub paths: LauncherPaths,
    pub launcher_root: String,
    pub data_source: DataSource,
    pub result_data_sources: Option<hp2_launcher::model::DataSourceConfiguration>,
    pub result_settings: Option<LauncherSettings>,
    pub retail_root: String,
    pub retail_data_root: String,
    pub prototype_root: String,
    pub saves: Vec<SaveRecord>,
    pub selected_save: i32,
    pub error: Option<String>,
    pub data_message: Option<String>,
    pub action: Option<LaunchSelection>,
    pub finished: bool,
    pub needs_game_data: bool,
    pub modes: Vec<hp2_launcher::model::DisplayResolution>,
    tab: usize,
    import_progress: Option<std::sync::Arc<hp2_import::output::Progress>>,
    import_task: Option<std::thread::JoinHandle<()>>,
    importing: bool,
    import_done: std::sync::Arc<std::sync::atomic::AtomicBool>,
    import_succeeded: std::sync::Arc<std::sync::atomic::AtomicBool>,
    import_result: std::sync::Arc<std::sync::Mutex<ImportResult>>,
    thumbnail_path: String,
    thumbnail: Option<egui::TextureHandle>,
}

impl LauncherUiState {
    pub fn new(
        settings: LauncherSettings,
        paths: LauncherPaths,
        launcher_root: String,
        data_source: DataSource,
        retail_root: String,
        prototype_root: String,
        saves: Vec<SaveRecord>,
        needs_game_data: bool,
    ) -> Self {
        let data_source_options = vec![
            DataSourceOption {
                source: DataSource::Retail,
                root: retail_root.clone(),
                available: !retail_root.is_empty(),
                error: None,
            },
            DataSourceOption {
                source: DataSource::Prototype,
                root: prototype_root.clone(),
                available: !prototype_root.is_empty(),
                error: None,
            },
        ];
        let retail_data_root = String::new();
        let mut state = Self {
            settings,
            paths,
            launcher_root,
            data_source,
            retail_root,
            retail_data_root,
            prototype_root,
            saves,
            data_source_options,
            selected_save: -1,
            error: None,
            data_message: None,
            action: Some(LaunchSelection::default()),
            finished: false,
            needs_game_data,
            tab: 0,
            modes: Vec::new(),
            import_progress: None,
            import_task: None,
            importing: false,
            import_done: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            import_succeeded: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            import_result: std::sync::Arc::new(std::sync::Mutex::new(ImportResult::default())),
            thumbnail_path: String::new(),
            thumbnail: None,
            result_data_sources: None,
            result_settings: None,
        };
        state.import_action(
            hp2_launcher::import_session::Hp2LauncherImportSessionAction::Initialize,
        );
        state
    }

    fn update_import(&mut self) {
        if self
            .import_task
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
        {
            self.import_action(
                hp2_launcher::import_session::Hp2LauncherImportSessionAction::FinishIfReady,
            );
        }
    }
    fn persist_settings(&mut self) {
        self.result_settings = Some(self.settings.clone());
        if let Err(e) = orchestrate::commit_launcher_settings(&self.paths, &self.settings) {
            self.error = Some(e);
        }
    }
    fn import_action(
        &mut self,
        action: hp2_launcher::import_session::Hp2LauncherImportSessionAction,
    ) {
        let mut frame = RetailImportFrame {
            state: self,
            source: String::new(),
            destination: String::new(),
            root: String::new(),
            canonical_root: String::new(),
            root_error: String::new(),
            progress: std::sync::Arc::new(hp2_import::output::Progress::default()),
        };
        let services = hp2_launcher::import_session::Hp2LauncherImportSessionServices {
            context: (&mut frame as *mut RetailImportFrame).cast(),
            call: retail_import_call,
            message: retail_import_message,
            pick_archive: retail_import_pick_archive,
            import_locals: retail_import_locals,
            publish_import_result: retail_import_result,
        };
        if let Err(status) = unsafe { hp2_launcher::import_session::execute(&services, action) } {
            self.error = Some(format!(
                "The retail import action failed with status {status}."
            ));
        }
    }

    pub fn draw(&mut self, root_ui: &mut Ui) -> RuntimeResult<Option<LaunchSelection>> {
        if self.finished {
            return Ok(self.action.take());
        }
        self.update_import();
        let before = self.settings.clone();
        egui::CentralPanel::default().show(root_ui, |ui| {
            ui.heading("Harry Potter 2");
            ui.separator();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    for (index, name) in ["Play", "Graphics", "Audio", "Gameplay", "Game Data"].iter().enumerate() {
                        if ui.add_sized([150.0, 44.0], egui::Button::new(*name).selected(self.tab == index)).clicked() { self.tab = index; }
                    }
                });
                ui.separator();
                ui.vertical(|ui| {
                    match self.tab {
                        0 => self.policy_settings(ui, hp2_launcher::presentation_policy::Hp2LauncherPresentationPhase::Play),
                        1 => self.policy_settings(ui, hp2_launcher::presentation_policy::Hp2LauncherPresentationPhase::Graphics),
                        2 => self.policy_settings(ui, hp2_launcher::presentation_policy::Hp2LauncherPresentationPhase::Audio),
                        3 => self.policy_settings(ui, hp2_launcher::presentation_policy::Hp2LauncherPresentationPhase::Gameplay),
                        _ => self.policy_settings(ui, hp2_launcher::presentation_policy::Hp2LauncherPresentationPhase::GameData),
                    }
                    if let Some(error) = self.error.as_ref() { ui.colored_label(egui::Color32::LIGHT_RED, error); }
                });
            });
        });
        if !Self::same_settings(&before, &self.settings) {
            self.persist_settings();
        }
        Ok(if self.finished {
            self.action.take()
        } else {
            None
        })
    }

    fn same_settings(a: &LauncherSettings, b: &LauncherSettings) -> bool {
        a.screen_mode == b.screen_mode
            && a.resolution.width == b.resolution.width
            && a.resolution.height == b.resolution.height
            && a.vertical_sync == b.vertical_sync
            && a.render_scale == b.render_scale
            && a.ui_scale == b.ui_scale
            && a.frame_rate_limit == b.frame_rate_limit
            && a.show_fps == b.show_fps
            && a.maintain_vertical_fov == b.maintain_vertical_fov
            && a.native_text == b.native_text
            && a.anti_aliasing_samples == b.anti_aliasing_samples
            && a.anisotropy == b.anisotropy
            && a.brightness == b.brightness
            && a.texture_detail == b.texture_detail
            && a.object_detail == b.object_detail
            && a.sound_enabled == b.sound_enabled
            && a.sound_volume == b.sound_volume
            && a.music_volume == b.music_volume
            && a.mouse_sensitivity == b.mouse_sensitivity
            && a.invert_mouse == b.invert_mouse
            && a.control_mode == b.control_mode
            && a.auto_center_camera == b.auto_center_camera
            && a.move_while_casting == b.move_while_casting
            && a.auto_quaff == b.auto_quaff
            && a.screen_flashes == b.screen_flashes
            && a.difficulty == b.difficulty
            && a.joystick_enabled == b.joystick_enabled
    }
    fn policy_settings(
        &mut self,
        ui: &mut Ui,
        phase: hp2_launcher::presentation_policy::Hp2LauncherPresentationPhase,
    ) {
        let mut frame = PolicyFrame {
            ui,
            state: self,
            disabled: false,
            labels: Vec::new(),
            label_refs: Vec::new(),
            date_seconds: 0,
            date_text: String::new(),
            date_display: String::new(),
            padding: Vec::new(),
            thumbnail_candidate: String::new(),
            thumbnail_pixels: None,
            full_width: false,
        };
        let services = hp2_launcher::presentation_policy::Hp2LauncherPresentationServices {
            context: (&mut frame as *mut PolicyFrame<'_>).cast(),
            call: policy_call,
        };
        if unsafe { hp2_launcher::presentation_policy::execute(&services, phase) }.is_err() {
            frame.state.error = Some("Launcher settings UI policy failed.".into());
        }
    }
}

impl Drop for LauncherUiState {
    fn drop(&mut self) {
        if self.import_task.is_some() {
            self.import_action(hp2_launcher::import_session::Hp2LauncherImportSessionAction::Close);
        }
    }
}

fn format_save_date(seconds: i64) -> Option<String> {
    if seconds <= 0 {
        return None;
    }
    let timestamp = seconds as libc::time_t;
    let mut broken_down: libc::tm = unsafe { std::mem::zeroed() };
    let mut buffer = [0i8; 64];
    let success = unsafe {
        !libc::localtime_r(&timestamp, &mut broken_down).is_null()
            && libc::strftime(
                buffer.as_mut_ptr(),
                buffer.len(),
                b"%x %H:%M\0".as_ptr().cast(),
                &broken_down,
            ) != 0
    };
    if !success {
        return None;
    }
    let bytes = unsafe {
        std::slice::from_raw_parts(
            buffer.as_ptr().cast::<u8>(),
            buffer.iter().position(|b| *b == 0).unwrap_or(buffer.len()),
        )
    };
    Some(String::from_utf8_lossy(bytes).into_owned())
}

fn load_bmp(path: &str) -> Result<egui::ColorImage, String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    if bytes.get(0..2) != Some(b"BM") || bytes.len() < 54 {
        return Err("invalid BMP image".into());
    }
    let u16_at = |offset: usize| -> Option<u16> {
        Some(u16::from_le_bytes(
            bytes.get(offset..offset + 2)?.try_into().ok()?,
        ))
    };
    let u32_at = |offset: usize| -> Option<u32> {
        Some(u32::from_le_bytes(
            bytes.get(offset..offset + 4)?.try_into().ok()?,
        ))
    };
    let i32_at = |offset: usize| -> Option<i32> {
        Some(i32::from_le_bytes(
            bytes.get(offset..offset + 4)?.try_into().ok()?,
        ))
    };
    let offset = usize::try_from(u32_at(10).ok_or("invalid BMP pixel offset")?)
        .map_err(|_| "BMP pixel offset overflow")?;
    let dib_size = u32_at(14).ok_or("invalid BMP header")?;
    let width = i32_at(18).ok_or("invalid BMP width")?;
    let signed_height = i32_at(22).ok_or("invalid BMP height")?;
    let planes = u16_at(26).ok_or("invalid BMP planes")?;
    let bits = u16_at(28).ok_or("invalid BMP bit depth")?;
    let compression = u32_at(30).ok_or("invalid BMP compression")?;
    if dib_size < 40
        || width <= 0
        || signed_height == 0
        || planes != 1
        || !matches!(bits, 24 | 32)
        || compression != 0
    {
        return Err("unsupported BMP format".into());
    }
    let width = usize::try_from(width).map_err(|_| "BMP width overflow")?;
    let height =
        usize::try_from(signed_height.unsigned_abs()).map_err(|_| "BMP height overflow")?;
    let pixels_count = width.checked_mul(height).ok_or("BMP dimensions overflow")?;
    let row_bits = width.checked_mul(bits as usize).ok_or("BMP row overflow")?;
    let stride = row_bits.checked_add(31).ok_or("BMP row overflow")? / 32 * 4;
    let data_len = stride.checked_mul(height).ok_or("BMP data overflow")?;
    if offset
        .checked_add(data_len)
        .is_none_or(|end| end > bytes.len())
    {
        return Err("truncated BMP pixels".into());
    }
    let mut pixels = vec![egui::Color32::TRANSPARENT; pixels_count];
    let pixel_bytes = (bits / 8) as usize;
    for y in 0..height {
        let source_y = if signed_height < 0 { y } else { height - y - 1 };
        let row = offset + source_y * stride;
        for x in 0..width {
            let i = row + x * pixel_bytes;
            let alpha = if pixel_bytes == 4 { bytes[i + 3] } else { 255 };
            pixels[y * width + x] =
                egui::Color32::from_rgba_unmultiplied(bytes[i + 2], bytes[i + 1], bytes[i], alpha);
        }
    }
    Ok(egui::ColorImage::new([width, height], pixels))
}
struct PolicyFrame<'a> {
    ui: &'a mut Ui,
    state: &'a mut LauncherUiState,
    disabled: bool,
    labels: Vec<String>,
    label_refs: Vec<usize>,
    date_seconds: i64,
    date_text: String,
    date_display: String,
    padding: Vec<egui::Vec2>,
    thumbnail_candidate: String,
    thumbnail_pixels: Option<egui::ColorImage>,
    full_width: bool,
}

unsafe extern "C" fn policy_call(
    raw: *mut std::ffi::c_void,
    leaf: hp2_launcher::presentation_policy::Hp2LauncherPresentationLeaf,
    index: usize,
    integer: i64,
    real: f64,
    text: *const u8,
    choices: *const u8,
    out: *mut hp2_launcher::presentation_policy::Hp2LauncherPresentationValue,
) -> i32 {
    use hp2_launcher::presentation_policy::Hp2LauncherPresentationLeaf as L;
    let nested = match leaf {
        L::ScopeResolution => {
            Some(hp2_launcher::presentation_policy::Hp2LauncherPresentationPhase::Resolution)
        }
        L::ScopeSaves => {
            Some(hp2_launcher::presentation_policy::Hp2LauncherPresentationPhase::Saves)
        }
        L::ScopeThumbnail => {
            Some(hp2_launcher::presentation_policy::Hp2LauncherPresentationPhase::Thumbnail)
        }
        _ => None,
    };
    if let Some(phase) = nested {
        let services = hp2_launcher::presentation_policy::Hp2LauncherPresentationServices {
            context: raw,
            call: policy_call,
        };
        if unsafe { hp2_launcher::presentation_policy::execute(&services, phase) }.is_err() {
            return 1;
        }
        unsafe {
            out.write(hp2_launcher::presentation_policy::Hp2LauncherPresentationValue::default());
        }
        return 0;
    }
    if matches!(leaf, L::TextSelectedDate) {
        {
            let frame = unsafe { &mut *raw.cast::<PolicyFrame<'_>>() };
            frame.date_seconds = usize::try_from(frame.state.selected_save)
                .ok()
                .and_then(|index| frame.state.saves.get(index))
                .map_or(0, |save| save.modified_seconds);
        }
        let services = hp2_launcher::presentation_policy::Hp2LauncherPresentationServices {
            context: raw,
            call: policy_call,
        };
        if unsafe {
            hp2_launcher::presentation_policy::execute(
                &services,
                hp2_launcher::presentation_policy::Hp2LauncherPresentationPhase::Date,
            )
        }
        .is_err()
        {
            return 1;
        }
        let frame = unsafe { &mut *raw.cast::<PolicyFrame<'_>>() };
        frame.ui.label(&frame.date_display);
        unsafe {
            out.write(hp2_launcher::presentation_policy::Hp2LauncherPresentationValue::default());
        }
        return 0;
    }
    if matches!(leaf, L::DateFormat) {
        let frame = unsafe { &mut *raw.cast::<PolicyFrame<'_>>() };
        frame.date_text = format_save_date(frame.date_seconds).unwrap_or_default();
        let services = hp2_launcher::presentation_policy::Hp2LauncherPresentationServices {
            context: raw,
            call: policy_call,
        };
        if unsafe {
            hp2_launcher::presentation_policy::execute(
                &services,
                hp2_launcher::presentation_policy::Hp2LauncherPresentationPhase::DateFormatted,
            )
        }
        .is_err()
        {
            return 1;
        }
        unsafe {
            out.write(hp2_launcher::presentation_policy::Hp2LauncherPresentationValue::default());
        }
        return 0;
    }
    let frame = unsafe { &mut *raw.cast::<PolicyFrame<'_>>() };
    let text = unsafe { std::ffi::CStr::from_ptr(text.cast()).to_bytes() };
    let mut value = hp2_launcher::presentation_policy::Hp2LauncherPresentationValue::default();
    let s = &mut frame.state.settings;
    match leaf {
        L::CapsScreenModes | L::CapsResolutions => value.integer = 1,
        L::ModeCount => value.integer = frame.state.modes.len() as i64,
        L::ModeWidth => value.integer = frame.state.modes.get(index).map_or(0, |m| m.width as i64),
        L::ModeHeight => {
            value.integer = frame.state.modes.get(index).map_or(0, |m| m.height as i64)
        }
        L::ResolutionWidth => value.integer = s.resolution.width as i64,
        L::ResolutionHeight => value.integer = s.resolution.height as i64,
        L::ModeLabelEmpty => {
            value.integer = frame
                .state
                .modes
                .get(index)
                .is_none_or(|m| m.label.is_empty()) as i64
        }
        L::AppendModeLabel => {
            if let Some(mode) = frame.state.modes.get(index) {
                frame.labels.push(mode.label.clone());
            }
        }
        L::AppendModeDimensions => {
            if let Some(mode) = frame.state.modes.get(index) {
                frame
                    .labels
                    .push(format!("{} x {}", mode.width, mode.height));
            }
        }
        L::AppendLabelRef => {
            if frame.labels.is_empty() {
                return 1;
            }
            frame.label_refs.push(frame.labels.len() - 1);
        }
        L::FullWidth => frame.full_width = true,
        L::ResolutionCombo => {
            let mut selected = integer.max(0) as usize;
            let selected_label = frame
                .label_refs
                .get(selected)
                .and_then(|label| frame.labels.get(*label))
                .map(String::as_str)
                .unwrap_or("800 × 600");
            frame.ui.label("Resolution");
            let mut combo =
                egui::ComboBox::from_id_salt("Resolution").selected_text(selected_label);
            if frame.full_width {
                combo = combo.width(frame.ui.available_width());
                frame.full_width = false;
            }
            combo.show_ui(frame.ui, |ui| {
                for (i, label_ref) in frame.label_refs.iter().enumerate() {
                    if let Some(label) = frame.labels.get(*label_ref) {
                        ui.selectable_value(&mut selected, i, label);
                    }
                }
            });
            value.integer = if selected as i64 == integer {
                -1
            } else {
                selected as i64
            };
        }
        L::SetResolution => {
            if let Some(mode) = frame.state.modes.get(index) {
                s.resolution = mode.clone();
            }
        }
        L::GetScreenMode => value.integer = s.screen_mode as i64,
        L::SetScreenMode => {
            s.screen_mode = [
                hp2_launcher::model::ScreenMode::Windowed,
                hp2_launcher::model::ScreenMode::Fullscreen,
                hp2_launcher::model::ScreenMode::BorderlessDesktop,
            ]
            .get(integer as usize)
            .copied()
            .unwrap_or_default()
        }
        L::GetRenderScale => value.real = s.render_scale,
        L::SetRenderScale => s.render_scale = real,
        L::GetUiScale => value.real = s.ui_scale,
        L::SetUiScale => s.ui_scale = real,
        L::GetFps => value.integer = s.frame_rate_limit as i64,
        L::SetFps => s.frame_rate_limit = integer as i32,
        L::GetAa => value.integer = s.anti_aliasing_samples as i64,
        L::SetAa => s.anti_aliasing_samples = integer as i32,
        L::GetAniso => value.integer = s.anisotropy as i64,
        L::SetAniso => s.anisotropy = integer as i32,
        L::GetBrightness => value.real = s.brightness,
        L::SetBrightness => s.brightness = real,
        L::GetTexture => value.integer = s.texture_detail as i64,
        L::SetTexture => {
            s.texture_detail = [
                hp2_launcher::model::TextureDetail::Low,
                hp2_launcher::model::TextureDetail::Medium,
                hp2_launcher::model::TextureDetail::High,
            ]
            .get(integer as usize)
            .copied()
            .unwrap_or_default()
        }
        L::GetObject => value.integer = s.object_detail as i64,
        L::SetObject => {
            s.object_detail = [
                hp2_launcher::model::ObjectDetail::VeryLow,
                hp2_launcher::model::ObjectDetail::Low,
                hp2_launcher::model::ObjectDetail::Medium,
                hp2_launcher::model::ObjectDetail::High,
                hp2_launcher::model::ObjectDetail::VeryHigh,
            ]
            .get(integer as usize)
            .copied()
            .unwrap_or_default()
        }
        L::GetEffects => value.real = s.sound_volume,
        L::SetEffects => s.sound_volume = real,
        L::GetMusic => value.real = s.music_volume,
        L::SetMusic => s.music_volume = real,
        L::GetSensitivity => value.real = s.mouse_sensitivity,
        L::SetSensitivity => s.mouse_sensitivity = real,
        L::GetControls => value.integer = s.control_mode as i64,
        L::SetControls => {
            s.control_mode = [
                hp2_launcher::model::ControlMode::Classic,
                hp2_launcher::model::ControlMode::Modern,
            ]
            .get(integer as usize)
            .copied()
            .unwrap_or_default()
        }
        L::GetDifficulty => value.integer = s.difficulty as i64,
        L::SetDifficulty => {
            s.difficulty = [
                hp2_launcher::model::Difficulty::Easy,
                hp2_launcher::model::Difficulty::Medium,
                hp2_launcher::model::Difficulty::Hard,
            ]
            .get(integer as usize)
            .copied()
            .unwrap_or_default()
        }
        L::CheckVsync => {
            frame.ui.checkbox(&mut s.vertical_sync, "Vertical Sync");
        }
        L::CheckShowFps => {
            frame.ui.checkbox(&mut s.show_fps, "Show FPS");
        }
        L::CheckVerticalFov => {
            frame
                .ui
                .checkbox(&mut s.maintain_vertical_fov, "Maintain vertical FOV");
        }
        L::CheckNativeText => {
            frame.ui.checkbox(&mut s.native_text, "Native text");
        }
        L::CheckSound => {
            frame.ui.checkbox(&mut s.sound_enabled, "Sound enabled");
        }
        L::CheckInvert => {
            frame.ui.checkbox(&mut s.invert_mouse, "Invert mouse");
        }
        L::CheckJoystick => {
            frame.ui.checkbox(&mut s.joystick_enabled, "Joystick");
        }
        L::CheckCenter => {
            frame
                .ui
                .checkbox(&mut s.auto_center_camera, "Auto-center camera");
        }
        L::CheckCasting => {
            frame
                .ui
                .checkbox(&mut s.move_while_casting, "Move while casting");
        }
        L::CheckQuaff => {
            frame.ui.checkbox(&mut s.auto_quaff, "Auto-quaff potions");
        }
        L::CheckFlashes => {
            frame.ui.checkbox(&mut s.screen_flashes, "Screen flashes");
        }
        L::SoundEnabled => value.integer = s.sound_enabled as i64,
        L::TextSavedGames => {
            frame.ui.label("Saved games");
        }
        L::TextNoSaves => {
            frame.ui.label("No saved games found.");
        }
        L::ValidationEmpty => value.integer = i64::from(frame.state.error.is_none()),
        L::TextValidation => {
            if let Some(error) = frame.state.error.as_ref() {
                frame.ui.label(error);
            }
        }
        L::SaveCount | L::SaveCountInt => value.integer = frame.state.saves.len() as i64,
        L::SelectedSave => value.integer = frame.state.selected_save as i64,
        L::SaveBytes => value.integer = frame.state.saves.get(index).map_or(0, |save| save.size),
        L::LabelsReserve => {
            frame.labels.clear();
            frame.label_refs.clear();
            frame.labels.reserve(index);
            frame.label_refs.reserve(index);
        }
        L::AppendSaveLabel => {
            if let Some(save) = frame.state.saves.get(index) {
                let date = format_save_date(save.modified_seconds)
                    .unwrap_or_else(|| "Date unavailable".into());
                frame.labels.push(format!(
                    "{}  —  {}  —  {} KB",
                    save.display_name, date, integer
                ));
            }
        }
        L::SaveCombo => {
            let mut selected = frame.state.selected_save.max(0) as usize;
            frame.ui.label("Save");
            egui::ComboBox::from_id_salt("Save")
                .selected_text(
                    frame
                        .labels
                        .get(selected)
                        .map(String::as_str)
                        .unwrap_or("Select save"),
                )
                .show_ui(frame.ui, |ui| {
                    for (i, label) in frame.labels.iter().enumerate() {
                        ui.selectable_value(&mut selected, i, label);
                    }
                });
            frame.state.selected_save = selected as i32;
        }
        L::HasSave => {
            value.integer = i64::from(
                frame.state.selected_save >= 0
                    && (frame.state.selected_save as usize) < frame.state.saves.len(),
            )
        }
        L::SelectedSource => value.integer = frame.state.data_source as i64,
        L::OptionCount => value.integer = frame.state.data_source_options.len() as i64,
        L::OptionSource => {
            value.integer = frame
                .state
                .data_source_options
                .get(index)
                .map_or(-1, |option| option.source as i64)
        }
        L::OptionRootEmpty => {
            value.integer = i64::from(
                frame
                    .state
                    .data_source_options
                    .get(index)
                    .is_none_or(|option| option.root.is_empty()),
            )
        }
        L::TextOptionRoot => {
            if let Some(option) = frame.state.data_source_options.get(index) {
                frame.ui.label(format!("Folder: {}", option.root));
            }
        }
        L::TextOptionUnconfigured => {
            frame.ui.label("Folder: (not configured)");
        }
        L::OptionAvailable => {
            value.integer = i64::from(
                frame
                    .state
                    .data_source_options
                    .get(index)
                    .is_some_and(|option| option.available),
            )
        }
        L::TextAvailable => {
            frame
                .ui
                .colored_label(egui::Color32::from_rgb(51, 217, 77), "Available");
        }
        L::TextUnavailable => {
            frame
                .ui
                .colored_label(egui::Color32::from_rgb(255, 140, 77), "Unavailable");
        }
        L::OptionErrorEmpty => {
            value.integer = i64::from(
                frame
                    .state
                    .data_source_options
                    .get(index)
                    .is_none_or(|option| option.error.is_none()),
            )
        }
        L::TextOptionError => {
            if let Some(error) = frame
                .state
                .data_source_options
                .get(index)
                .and_then(|option| option.error.as_ref())
            {
                frame.ui.label(error);
            }
        }
        L::CapsPicker | L::CapsReveal => value.integer = 1,
        L::LogEmpty => {
            value.integer = i64::from(
                !std::path::Path::new(&frame.state.launcher_root)
                    .join("Launcher.log")
                    .exists(),
            )
        }
        L::RevealUser => hp2_platform::reveal::reveal_path(frame.state.paths.user_root.as_bytes()),
        L::RevealLog => {
            let path = std::path::Path::new(&frame.state.launcher_root).join("Launcher.log");
            hp2_platform::reveal::reveal_path(path.to_string_lossy().as_bytes());
        }
        L::Importing => value.integer = i64::from(frame.state.importing),
        L::FilesDone => {
            value.integer = frame.state.import_progress.as_ref().map_or(0, |progress| {
                progress
                    .files_done
                    .load(std::sync::atomic::Ordering::SeqCst) as i64
            })
        }
        L::FilesTotal => {
            value.integer = frame.state.import_progress.as_ref().map_or(0, |progress| {
                progress
                    .files_total
                    .load(std::sync::atomic::Ordering::SeqCst) as i64
            })
        }
        L::Progress => {
            let label = String::from_utf8_lossy(text);
            frame
                .ui
                .add(egui::ProgressBar::new(real as f32).text(label));
        }
        L::DataMessageEmpty => value.integer = i64::from(frame.state.data_message.is_none()),
        L::TextDataMessage => {
            if let Some(message) = frame.state.data_message.as_ref() {
                frame.ui.label(message);
            }
        }
        L::ButtonPadding => {
            frame.padding.push(frame.ui.spacing().button_padding);
            frame.ui.spacing_mut().button_padding = egui::vec2(12.0, 13.0);
        }
        L::BeginDisabled => frame.disabled = integer != 0,
        L::EndDisabled => frame.disabled = false,
        L::Button => {
            let label = String::from_utf8_lossy(text);
            value.integer = i64::from(
                frame
                    .ui
                    .add_enabled(
                        !frame.disabled,
                        egui::Button::new(label.as_ref())
                            .min_size(egui::vec2(frame.ui.available_width(), 44.0)),
                    )
                    .clicked(),
            );
        }
        L::PopStyle => {
            if let Some(padding) = frame.padding.pop() {
                frame.ui.spacing_mut().button_padding = padding;
            }
        }
        L::ImportAction => {
            let action = match integer {
                1 => hp2_launcher::import_session::Hp2LauncherImportSessionAction::Start,
                2 => hp2_launcher::import_session::Hp2LauncherImportSessionAction::Worker,
                3 => hp2_launcher::import_session::Hp2LauncherImportSessionAction::FinishIfReady,
                4 => hp2_launcher::import_session::Hp2LauncherImportSessionAction::Cancel,
                5 => hp2_launcher::import_session::Hp2LauncherImportSessionAction::Close,
                6 => hp2_launcher::import_session::Hp2LauncherImportSessionAction::RefreshRetail,
                7 => hp2_launcher::import_session::Hp2LauncherImportSessionAction::PickRetailFolder,
                8 => hp2_launcher::import_session::Hp2LauncherImportSessionAction::PickRetailArchive,
                9 => hp2_launcher::import_session::Hp2LauncherImportSessionAction::PickPrototypeFolder,
                10 => hp2_launcher::import_session::Hp2LauncherImportSessionAction::SelectRetail,
                11 => hp2_launcher::import_session::Hp2LauncherImportSessionAction::SelectPrototype,
                12 => hp2_launcher::import_session::Hp2LauncherImportSessionAction::CompleteContinue,
                13 => hp2_launcher::import_session::Hp2LauncherImportSessionAction::CompleteNewGame,
                14 => hp2_launcher::import_session::Hp2LauncherImportSessionAction::CompleteQuit,
                15 => hp2_launcher::import_session::Hp2LauncherImportSessionAction::CompleteError,
                16 => hp2_launcher::import_session::Hp2LauncherImportSessionAction::PrepareSavePicker,
                _ => return 1,
            };
            frame.state.import_action(action);
        }
        L::DateSeconds => value.integer = frame.date_seconds,
        L::DateCount => value.integer = frame.date_text.len() as i64,
        L::DateUnavailable => {
            frame.date_text = "Date unavailable".into();
            frame.date_display.clone_from(&frame.date_text);
        }
        L::DatePublish => frame.date_display.clone_from(&frame.date_text),
        L::Thumbnail => value.integer = i64::from(frame.state.thumbnail.is_some()),
        L::ThumbnailDestroy | L::ThumbnailClear => frame.state.thumbnail = None,
        L::ThumbnailCaptureSelected => {
            if let Some(save) = frame.state.saves.get(index) {
                frame.thumbnail_candidate = save.thumbnail_path.clone();
            }
        }
        L::ThumbnailSame => {
            value.integer = i64::from(frame.state.thumbnail_path == frame.thumbnail_candidate)
        }
        L::ThumbnailPublish => frame
            .state
            .thumbnail_path
            .clone_from(&frame.thumbnail_candidate),
        L::ThumbnailPathEmpty => value.integer = i64::from(frame.thumbnail_candidate.is_empty()),
        L::ThumbnailLoad => match load_bmp(&frame.thumbnail_candidate) {
            Ok(image) => {
                frame.thumbnail_pixels = Some(image);
                value.integer = 1;
            }
            Err(_) => {
                frame.thumbnail_pixels = None;
            }
        },
        L::ThumbnailCreate => {
            if let Some(image) = frame.thumbnail_pixels.take() {
                frame.state.thumbnail = Some(frame.ui.ctx().load_texture(
                    "launcher-save-thumbnail",
                    image,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }
        L::ThumbnailFree => frame.thumbnail_pixels = None,
        L::ThumbnailImage => {
            if let Some(texture) = frame.state.thumbnail.as_ref() {
                frame
                    .ui
                    .add(egui::Image::new((texture.id(), texture.size_vec2())));
            }
        }
        L::Combo => {
            // Only Combo receives a double-NUL choice list. Other leaves pass
            // a single empty C string, which must not be scanned as a list.
            let mut choice_len = 0usize;
            let mut zeros = 0usize;
            while zeros < 2 {
                let byte = unsafe { *choices.add(choice_len) };
                choice_len += 1;
                zeros = if byte == 0 { zeros + 1 } else { 0 };
            }
            let choices = unsafe { std::slice::from_raw_parts(choices, choice_len - 2) };
            let options = || {
                choices
                    .split(|byte| *byte == 0)
                    .take_while(|choice| !choice.is_empty())
            };
            let current = options().nth(integer.max(0) as usize).unwrap_or_default();
            let mut selected = integer.max(0) as usize;
            let label = String::from_utf8_lossy(text);
            frame.ui.label(label.as_ref());
            let mut combo = egui::ComboBox::from_id_salt(label.as_ref())
                .selected_text(String::from_utf8_lossy(current));
            if frame.full_width {
                combo = combo.width(frame.ui.available_width());
                frame.full_width = false;
            }
            combo.show_ui(frame.ui, |ui| {
                for (i, label) in options().enumerate() {
                    ui.selectable_value(&mut selected, i, String::from_utf8_lossy(label));
                }
            });
            value.integer = if selected as i64 == integer {
                -1
            } else {
                selected as i64
            };
        }
        L::SliderBrightness | L::SliderEffects | L::SliderMusic | L::SliderSensitivity => {
            let (min, max) = match leaf {
                L::SliderBrightness => (0.1, 1.0),
                L::SliderSensitivity => (0.2, 10.0),
                _ => (0.0, 1.0),
            };
            let label = match leaf {
                L::SliderBrightness => "Brightness",
                L::SliderEffects => "Effects volume",
                L::SliderMusic => "Music volume",
                _ => "Mouse sensitivity",
            };
            let mut selected = real as f32;
            let response = frame.ui.add_enabled(
                !frame.disabled,
                egui::Slider::new(&mut selected, min..=max).text(label),
            );
            value.integer = i64::from(response.changed());
            value.real = selected as f64;
        }
        _ => return 1,
    }
    unsafe {
        out.write(value);
    }
    0
}

pub fn run_window<T>(
    title: &str,
    width: i32,
    height: i32,
    draw: impl FnMut(&mut Ui) -> RuntimeResult<Option<T>>,
) -> RuntimeResult<Option<T>> {
    crate::launcher_window::run(title, width, height, draw)
}

pub(crate) fn validate_root(candidate: &str) -> Result<String, String> {
    let outcome =
        hp2_bootstrap_paths::bootstrap_paths::run(1, candidate.as_bytes(), b"", &[], |_, _| 1);
    if outcome.success {
        String::from_utf8(outcome.text[0].clone()).map_err(|_| "Validated root is not UTF-8".into())
    } else {
        Err(String::from_utf8_lossy(&outcome.text[1]).into_owned())
    }
}

impl Default for LauncherUiState {
    fn default() -> Self {
        Self::new(
            LauncherSettings::default(),
            LauncherPaths::default(),
            String::new(),
            DataSource::Retail,
            String::new(),
            String::new(),
            Vec::new(),
            true,
        )
    }
}
