/*=============================================================================
	HP2LauncherAdapter.cpp: Thin C++ implementation of HP2LauncherStore.h's
	and HP2LaunchPolicy.h's declared functions over the Rust hp2_rs_launcher_*
	FFI surface (crates/hp2-launcher via crates/hp2-ffi). Replaces
	HP2LauncherStore.cpp and HP2LaunchPolicy.cpp, which this project's whole
	non-UI launcher logic was ported from -- see crates/hp2-launcher's own
	module docs for the full port history and verification record.

	Both headers are UNCHANGED: HP2Launcher.cpp, SDLLaunch.cpp, and
	Tests/LauncherTests.cpp (the "native_launcher_contract" ctest, this
	port's independent oracle) all compile against them exactly as before,
	calling the same signatures -- only the implementation underneath now
	runs in Rust.

	ParseLauncherAutomationRequest/BuildLauncherAutomationResult
	(HP2LaunchPolicy.h) are declared but intentionally left undefined here,
	matching the real C++ exactly: neither has a body in the file this
	replaces either, and grep across the whole repository confirms nothing
	calls them (vestigial declarations).
=============================================================================*/
#include "HP2LauncherStore.h"
#include "HP2LaunchPolicy.h"
#include "hp2_rs.h"

#include <cstring>

namespace HP2Launcher
{
namespace
{
void ErrorSinkToString(void* ctx, const uint8_t* text, size_t len)
{
	static_cast<std::string*>(ctx)->assign(reinterpret_cast<const char*>(text), len);
}

FfiLauncherSettings ToFfi(const LauncherSettings& settings)
{
	FfiLauncherSettings ffi{};
	ffi.screen_mode = static_cast<int32_t>(settings.screenMode);
	ffi.resolution_width = settings.resolution.width;
	ffi.resolution_height = settings.resolution.height;
	ffi.render_backend = static_cast<int32_t>(settings.renderBackend);
	ffi.vertical_sync = settings.verticalSync;
	ffi.render_scale = settings.renderScale;
	ffi.ui_scale = settings.uiScale;
	ffi.frame_rate_limit = settings.frameRateLimit;
	ffi.show_fps = settings.showFPS;
	ffi.maintain_vertical_fov = settings.maintainVerticalFOV;
	ffi.native_text = settings.nativeText;
	ffi.anti_aliasing_samples = settings.antiAliasingSamples;
	ffi.anisotropy = settings.anisotropy;
	ffi.brightness = settings.brightness;
	ffi.texture_detail = static_cast<int32_t>(settings.textureDetail);
	ffi.object_detail = static_cast<int32_t>(settings.objectDetail);
	ffi.sound_enabled = settings.soundEnabled;
	ffi.sound_volume = settings.soundVolume;
	ffi.music_volume = settings.musicVolume;
	ffi.mouse_sensitivity = settings.mouseSensitivity;
	ffi.invert_mouse = settings.invertMouse;
	ffi.control_mode = static_cast<int32_t>(settings.controlMode);
	ffi.auto_center_camera = settings.autoCenterCamera;
	ffi.move_while_casting = settings.moveWhileCasting;
	ffi.auto_quaff = settings.autoQuaff;
	ffi.screen_flashes = settings.screenFlashes;
	ffi.difficulty = static_cast<int32_t>(settings.difficulty);
	ffi.joystick_enabled = settings.joystickEnabled;
	return ffi;
}

LauncherSettings FromFfi(const FfiLauncherSettings& ffi)
{
	LauncherSettings settings;
	settings.screenMode = static_cast<ScreenMode>(ffi.screen_mode);
	settings.resolution.width = ffi.resolution_width;
	settings.resolution.height = ffi.resolution_height;
	settings.resolution.label =
		std::to_string(ffi.resolution_width) + " x " + std::to_string(ffi.resolution_height);
	settings.renderBackend = static_cast<RenderBackend>(ffi.render_backend);
	settings.verticalSync = ffi.vertical_sync;
	settings.renderScale = ffi.render_scale;
	settings.uiScale = ffi.ui_scale;
	settings.frameRateLimit = ffi.frame_rate_limit;
	settings.showFPS = ffi.show_fps;
	settings.maintainVerticalFOV = ffi.maintain_vertical_fov;
	settings.nativeText = ffi.native_text;
	settings.antiAliasingSamples = ffi.anti_aliasing_samples;
	settings.anisotropy = ffi.anisotropy;
	settings.brightness = ffi.brightness;
	settings.textureDetail = static_cast<TextureDetail>(ffi.texture_detail);
	settings.objectDetail = static_cast<ObjectDetail>(ffi.object_detail);
	settings.soundEnabled = ffi.sound_enabled;
	settings.soundVolume = ffi.sound_volume;
	settings.musicVolume = ffi.music_volume;
	settings.mouseSensitivity = ffi.mouse_sensitivity;
	settings.invertMouse = ffi.invert_mouse;
	settings.controlMode = static_cast<ControlMode>(ffi.control_mode);
	settings.autoCenterCamera = ffi.auto_center_camera;
	settings.moveWhileCasting = ffi.move_while_casting;
	settings.autoQuaff = ffi.auto_quaff;
	settings.screenFlashes = ffi.screen_flashes;
	settings.difficulty = static_cast<Difficulty>(ffi.difficulty);
	settings.joystickEnabled = ffi.joystick_enabled;
	return settings;
}

void SaveSinkToVector(
	void* ctx,
	int32_t slot,
	int32_t saveIndex,
	bool usesSlotDirectory,
	const uint8_t* savePath,
	size_t savePathLen,
	const uint8_t* thumbnailPath,
	size_t thumbnailPathLen,
	const uint8_t* displayName,
	size_t displayNameLen,
	int64_t size,
	int64_t modifiedSeconds)
{
	auto* saves = static_cast<std::vector<SaveRecord>*>(ctx);
	SaveRecord record;
	record.slot = slot;
	record.saveIndex = saveIndex;
	record.usesSlotDirectory = usesSlotDirectory;
	record.savePath.assign(reinterpret_cast<const char*>(savePath), savePathLen);
	record.thumbnailPath.assign(reinterpret_cast<const char*>(thumbnailPath), thumbnailPathLen);
	record.displayName.assign(reinterpret_cast<const char*>(displayName), displayNameLen);
	record.size = size;
	record.modifiedSeconds = modifiedSeconds;
	saves->push_back(std::move(record));
}
} // namespace

bool LoadLauncherState(const LauncherPaths& paths, LauncherState& state, std::string& error)
{
	error.clear();
	state.saves.clear();
	FfiLauncherSettings ffiSettings{};
	const bool ok = hp2_rs_launcher_load_state(
		reinterpret_cast<const uint8_t*>(paths.userRoot.data()), paths.userRoot.size(),
		reinterpret_cast<const uint8_t*>(paths.systemRoot.data()), paths.systemRoot.size(),
		&ffiSettings, &SaveSinkToVector, &state.saves, &ErrorSinkToString, &error);
	if (!ok) return false;
	state.settings = FromFfi(ffiSettings);
	return true;
}

bool ValidateLauncherSettings(const LauncherSettings& settings, std::string& error)
{
	const FfiLauncherSettings ffi = ToFfi(settings);
	return hp2_rs_launcher_validate_settings(&ffi, &ErrorSinkToString, &error);
}

bool CommitLauncherSettings(const LauncherPaths& paths, const LauncherSettings& settings, std::string& error)
{
	const FfiLauncherSettings ffi = ToFfi(settings);
	return hp2_rs_launcher_commit_settings(
		reinterpret_cast<const uint8_t*>(paths.userRoot.data()), paths.userRoot.size(),
		reinterpret_cast<const uint8_t*>(paths.systemRoot.data()), paths.systemRoot.size(),
		&ffi, &ErrorSinkToString, &error);
}

bool LoadDataSourceConfiguration(
	const std::string& launcherRoot,
	DataSourceConfiguration& configuration,
	std::string& error)
{
	error.clear();
	configuration = DataSourceConfiguration();
	struct Ctx { DataSourceConfiguration* out; };
	Ctx ctx{&configuration};
	const bool ok = hp2_rs_launcher_load_data_sources(
		reinterpret_cast<const uint8_t*>(launcherRoot.data()), launcherRoot.size(),
		[](void* rawCtx, int32_t selected, const uint8_t* retail, size_t retailLen,
			const uint8_t* prototype, size_t prototypeLen)
		{
			auto* c = static_cast<Ctx*>(rawCtx);
			c->out->selected = static_cast<DataSource>(selected);
			c->out->retailRoot.assign(reinterpret_cast<const char*>(retail), retailLen);
			c->out->prototypeRoot.assign(reinterpret_cast<const char*>(prototype), prototypeLen);
		},
		&ctx, &ErrorSinkToString, &error);
	return ok;
}

bool CommitDataSourceConfiguration(
	const std::string& launcherRoot,
	const DataSourceConfiguration& configuration,
	std::string& error)
{
	return hp2_rs_launcher_commit_data_sources(
		reinterpret_cast<const uint8_t*>(launcherRoot.data()), launcherRoot.size(),
		static_cast<int32_t>(configuration.selected),
		reinterpret_cast<const uint8_t*>(configuration.retailRoot.data()), configuration.retailRoot.size(),
		reinterpret_cast<const uint8_t*>(configuration.prototypeRoot.data()), configuration.prototypeRoot.size(),
		&ErrorSinkToString, &error);
}

std::vector<LegacySettingValue> MigrateLegacySettings(
	const std::vector<LegacySettingValue>& legacyValues,
	std::vector<SettingsMigrationChange>& changes)
{
	changes.clear();
	std::vector<FfiLegacySettingValue> ffiValues;
	ffiValues.reserve(legacyValues.size());
	for (const LegacySettingValue& value : legacyValues)
	{
		FfiLegacySettingValue ffi{};
		ffi.section = reinterpret_cast<const uint8_t*>(value.section.data());
		ffi.section_len = value.section.size();
		ffi.key = reinterpret_cast<const uint8_t*>(value.key.data());
		ffi.key_len = value.key.size();
		ffi.value = reinterpret_cast<const uint8_t*>(value.value.data());
		ffi.value_len = value.value.size();
		ffiValues.push_back(ffi);
	}
	std::vector<LegacySettingValue> migrated;
	struct Ctx
	{
		std::vector<LegacySettingValue>* values;
		std::vector<SettingsMigrationChange>* changes;
	};
	Ctx ctx{&migrated, &changes};
	hp2_rs_launcher_migrate_legacy_settings(
		ffiValues.empty() ? nullptr : ffiValues.data(), ffiValues.size(),
		[](void* rawCtx, const uint8_t* section, size_t sectionLen, const uint8_t* key, size_t keyLen,
			const uint8_t* value, size_t valueLen)
		{
			auto* c = static_cast<Ctx*>(rawCtx);
			LegacySettingValue row;
			row.section.assign(reinterpret_cast<const char*>(section), sectionLen);
			row.key.assign(reinterpret_cast<const char*>(key), keyLen);
			row.value.assign(reinterpret_cast<const char*>(value), valueLen);
			c->values->push_back(std::move(row));
		},
		&ctx,
		[](void* rawCtx, const uint8_t* section, size_t sectionLen, const uint8_t* key, size_t keyLen,
			const uint8_t* previous, size_t previousLen, const uint8_t* next, size_t nextLen,
			const uint8_t* reason, size_t reasonLen)
		{
			auto* c = static_cast<Ctx*>(rawCtx);
			SettingsMigrationChange change;
			change.section.assign(reinterpret_cast<const char*>(section), sectionLen);
			change.key.assign(reinterpret_cast<const char*>(key), keyLen);
			change.previousValue.assign(reinterpret_cast<const char*>(previous), previousLen);
			change.newValue.assign(reinterpret_cast<const char*>(next), nextLen);
			change.reason.assign(reinterpret_cast<const char*>(reason), reasonLen);
			c->changes->push_back(std::move(change));
		},
		&ctx);
	return migrated;
}

bool CommitLaunchSelection(const std::string& launcherRoot, const LaunchSelection& selection, std::string& error)
{
	return hp2_rs_launcher_commit_selection(
		reinterpret_cast<const uint8_t*>(launcherRoot.data()), launcherRoot.size(),
		static_cast<int32_t>(selection.action), selection.hasSave,
		selection.save.slot, selection.save.saveIndex, selection.save.usesSlotDirectory,
		reinterpret_cast<const uint8_t*>(selection.save.savePath.data()), selection.save.savePath.size(),
		reinterpret_cast<const uint8_t*>(selection.save.thumbnailPath.data()), selection.save.thumbnailPath.size(),
		reinterpret_cast<const uint8_t*>(selection.save.displayName.data()), selection.save.displayName.size(),
		selection.save.size, selection.save.modifiedSeconds,
		&ErrorSinkToString, &error);
}

bool LoadLaunchSelection(const std::string& launcherRoot, LaunchSelection& selection, std::string& error)
{
	error.clear();
	// Default-constructed before the call: matches the real C++'s own
	// "selection still receives the safe Quit fallback" contract on
	// failure (LaunchSelection's default action is Quit) without
	// needing the FFI layer to re-communicate it separately.
	selection = LaunchSelection();
	struct Ctx { LaunchSelection* out; };
	Ctx ctx{&selection};
	return hp2_rs_launcher_load_selection(
		reinterpret_cast<const uint8_t*>(launcherRoot.data()), launcherRoot.size(),
		[](void* rawCtx, int32_t action, bool hasSave, int32_t slot, int32_t saveIndex, bool usesSlotDirectory,
			const uint8_t* savePath, size_t savePathLen, const uint8_t* thumbnailPath, size_t thumbnailPathLen,
			const uint8_t* displayName, size_t displayNameLen, int64_t size, int64_t modifiedSeconds)
		{
			auto* c = static_cast<Ctx*>(rawCtx);
			c->out->action = static_cast<LaunchAction>(action);
			c->out->hasSave = hasSave;
			c->out->save.slot = slot;
			c->out->save.saveIndex = saveIndex;
			c->out->save.usesSlotDirectory = usesSlotDirectory;
			c->out->save.savePath.assign(reinterpret_cast<const char*>(savePath), savePathLen);
			c->out->save.thumbnailPath.assign(reinterpret_cast<const char*>(thumbnailPath), thumbnailPathLen);
			c->out->save.displayName.assign(reinterpret_cast<const char*>(displayName), displayNameLen);
			c->out->save.size = size;
			c->out->save.modifiedSeconds = modifiedSeconds;
		},
		&ctx, &ErrorSinkToString, &error);
}

bool PrepareDataSourceProfile(
	const std::string& launcherRoot,
	DataSource source,
	std::string& profileRoot,
	std::string& error)
{
	profileRoot.clear();
	error.clear();
	std::string result;
	const bool ok = hp2_rs_launcher_prepare_profile(
		reinterpret_cast<const uint8_t*>(launcherRoot.data()), launcherRoot.size(),
		static_cast<int32_t>(source), &ErrorSinkToString, &result);
	if (ok)
		profileRoot = std::move(result);
	else
		error = std::move(result);
	return ok;
}

#ifdef HP2_LAUNCHER_TESTING
void SetLauncherPublishFailureForTesting(int publicationIndex)
{
	hp2_rs_launcher_set_publish_failure_for_testing(publicationIndex);
}
#endif

bool DiscoverSaves(const std::string& userRoot, std::vector<SaveRecord>& saves, std::string& error)
{
	saves.clear();
	error.clear();
	return hp2_rs_launcher_discover_saves(
		reinterpret_cast<const uint8_t*>(userRoot.data()), userRoot.size(),
		&SaveSinkToVector, &saves, &ErrorSinkToString, &error);
}

bool BuildSelectedCommand(const LaunchSelection& selection, std::string& utf8Prefix, std::string& error)
{
	utf8Prefix.clear();
	error.clear();
	std::string result;
	const bool ok = hp2_rs_launcher_build_selected_command(
		static_cast<int32_t>(selection.action), selection.hasSave,
		selection.save.slot, selection.save.saveIndex, selection.save.usesSlotDirectory,
		reinterpret_cast<const uint8_t*>(selection.save.savePath.data()), selection.save.savePath.size(),
		reinterpret_cast<const uint8_t*>(selection.save.thumbnailPath.data()), selection.save.thumbnailPath.size(),
		reinterpret_cast<const uint8_t*>(selection.save.displayName.data()), selection.save.displayName.size(),
		selection.save.size, selection.save.modifiedSeconds,
		&ErrorSinkToString, &result);
	if (ok)
		utf8Prefix = std::move(result);
	else
		error = std::move(result);
	return ok;
}

bool SerializeLaunchSelection(
	const LaunchSelection& selection,
	std::vector<LaunchSelectionField>& fields,
	std::string& error)
{
	fields.clear();
	error.clear();
	struct Ctx { std::vector<LaunchSelectionField>* out; };
	Ctx ctx{&fields};
	return hp2_rs_launcher_serialize_launch_selection(
		static_cast<int32_t>(selection.action), selection.hasSave,
		selection.save.slot, selection.save.saveIndex, selection.save.usesSlotDirectory,
		reinterpret_cast<const uint8_t*>(selection.save.savePath.data()), selection.save.savePath.size(),
		reinterpret_cast<const uint8_t*>(selection.save.thumbnailPath.data()), selection.save.thumbnailPath.size(),
		reinterpret_cast<const uint8_t*>(selection.save.displayName.data()), selection.save.displayName.size(),
		selection.save.size, selection.save.modifiedSeconds,
		[](void* rawCtx, const uint8_t* key, size_t keyLen, const uint8_t* value, size_t valueLen)
		{
			auto* c = static_cast<Ctx*>(rawCtx);
			LaunchSelectionField field;
			field.key.assign(reinterpret_cast<const char*>(key), keyLen);
			field.value.assign(reinterpret_cast<const char*>(value), valueLen);
			c->out->push_back(std::move(field));
		},
		&ctx, &ErrorSinkToString, &error);
}

bool DeserializeLaunchSelection(
	const std::vector<LaunchSelectionField>& fields,
	LaunchSelection& selection,
	std::string& error)
{
	error.clear();
	selection = LaunchSelection();
	std::vector<FfiLaunchSelectionField> ffiFields;
	ffiFields.reserve(fields.size());
	for (const LaunchSelectionField& field : fields)
	{
		FfiLaunchSelectionField ffi{};
		ffi.key = reinterpret_cast<const uint8_t*>(field.key.data());
		ffi.key_len = field.key.size();
		ffi.value = reinterpret_cast<const uint8_t*>(field.value.data());
		ffi.value_len = field.value.size();
		ffiFields.push_back(ffi);
	}
	struct Ctx { LaunchSelection* out; };
	Ctx ctx{&selection};
	return hp2_rs_launcher_deserialize_launch_selection(
		ffiFields.empty() ? nullptr : ffiFields.data(), ffiFields.size(),
		[](void* rawCtx, int32_t action, bool hasSave, int32_t slot, int32_t saveIndex, bool usesSlotDirectory,
			const uint8_t* savePath, size_t savePathLen, const uint8_t* thumbnailPath, size_t thumbnailPathLen,
			const uint8_t* displayName, size_t displayNameLen, int64_t size, int64_t modifiedSeconds)
		{
			auto* c = static_cast<Ctx*>(rawCtx);
			c->out->action = static_cast<LaunchAction>(action);
			c->out->hasSave = hasSave;
			c->out->save.slot = slot;
			c->out->save.saveIndex = saveIndex;
			c->out->save.usesSlotDirectory = usesSlotDirectory;
			c->out->save.savePath.assign(reinterpret_cast<const char*>(savePath), savePathLen);
			c->out->save.thumbnailPath.assign(reinterpret_cast<const char*>(thumbnailPath), thumbnailPathLen);
			c->out->save.displayName.assign(reinterpret_cast<const char*>(displayName), displayNameLen);
			c->out->save.size = size;
			c->out->save.modifiedSeconds = modifiedSeconds;
		},
		&ctx, &ErrorSinkToString, &error);
}

bool ShouldRunNativeLauncher(int argc, char* const argv[])
{
	std::vector<FfiStringSpan> spans;
	spans.reserve(static_cast<size_t>(argc));
	for (int i = 0; i < argc; ++i)
	{
		FfiStringSpan span{};
		span.ptr = reinterpret_cast<const uint8_t*>(argv[i]);
		span.len = argv[i] ? std::strlen(argv[i]) : 0;
		spans.push_back(span);
	}
	return hp2_rs_launcher_should_run_native_launcher(spans.empty() ? nullptr : spans.data(), spans.size());
}
}
