/*=============================================================================
	HP2MacLauncher.mm: Native macOS launcher bridge.
=============================================================================*/
#import <AppKit/AppKit.h>
#import <CoreGraphics/CoreGraphics.h>

#include "HP2MacLauncher.h"

#include <algorithm>
#include <cmath>
#include <set>
#include <string>
#include <utility>
#include <vector>

namespace
{
constexpr CGFloat LauncherWindowWidth = 760.0;
constexpr CGFloat LauncherWindowHeight = 640.0;
constexpr CGFloat LauncherMinimumWidth = 640.0;
constexpr CGFloat LauncherMinimumHeight = 520.0;
constexpr CGFloat OuterMargin = 24.0;
constexpr CGFloat SectionSpacing = 18.0;
constexpr CGFloat ItemSpacing = 10.0;
constexpr CGFloat RowSpacing = 12.0;
constexpr CGFloat FormLabelWidth = 176.0;
constexpr CGFloat PopupMinimumWidth = 260.0;
constexpr CGFloat SliderMinimumWidth = 196.0;
constexpr CGFloat ValueLabelWidth = 56.0;
constexpr CGFloat ThumbnailWidth = 120.0;
constexpr CGFloat ThumbnailHeight = 90.0;
constexpr CGFloat ActionButtonWidth = 120.0;
constexpr CGFloat TabContentInset = 20.0;
constexpr int MinimumResolutionWidth = 320;
constexpr int MinimumResolutionHeight = 320;
constexpr int MaximumResolutionDimension = 16384;
constexpr double MinimumBrightness = 0.1;
constexpr double MaximumBrightness = 1.0;
constexpr double MinimumMouseSensitivity = 0.2;
constexpr double MaximumMouseSensitivity = 10.0;

NSString* CocoaString(const std::string& value)
{
	NSString* string = [[NSString alloc]
		initWithBytes:value.data()
		length:value.size()
		encoding:NSUTF8StringEncoding];
	return string != nil ? string : @"";
}

std::string ResolutionLabel(int width, int height)
{
	return std::to_string(width) + " \xC3\x97 " + std::to_string(height);
}

bool IsRepresentableResolution(std::size_t width, std::size_t height)
{
	return width >= static_cast<std::size_t>(MinimumResolutionWidth) &&
		height >= static_cast<std::size_t>(MinimumResolutionHeight) &&
		width <= static_cast<std::size_t>(MaximumResolutionDimension) &&
		height <= static_cast<std::size_t>(MaximumResolutionDimension);
}

void AppendResolution(
	std::vector<HP2Launcher::DisplayResolution>& resolutions,
	std::set<std::pair<int, int>>& seen,
	int width,
	int height,
	const std::string& label = "")
{
	if (!IsRepresentableResolution(
			static_cast<std::size_t>(std::max(width, 0)),
			static_cast<std::size_t>(std::max(height, 0))) ||
		!seen.emplace(width, height).second)
	{
		return;
	}

	HP2Launcher::DisplayResolution resolution;
	resolution.width = width;
	resolution.height = height;
	resolution.label = label.empty() ? ResolutionLabel(width, height) : label;
	resolutions.push_back(std::move(resolution));
}

void SortResolutions(std::vector<HP2Launcher::DisplayResolution>& resolutions)
{
	std::stable_sort(
		resolutions.begin(),
		resolutions.end(),
		[](const HP2Launcher::DisplayResolution& first,
		   const HP2Launcher::DisplayResolution& second)
		{
			const long long firstArea =
				static_cast<long long>(first.width) * static_cast<long long>(first.height);
			const long long secondArea =
				static_cast<long long>(second.width) * static_cast<long long>(second.height);
			if (firstArea != secondArea)
			{
				return firstArea < secondArea;
			}
			if (first.width != second.width)
			{
				return first.width < second.width;
			}
			return first.height < second.height;
		});
}

bool WindowResolutionFitsVisibleFrame(int width, int height, NSScreen* screen)
{
	if (screen == nil)
	{
		return false;
	}
	const NSWindowStyleMask style =
		NSWindowStyleMaskTitled |
		NSWindowStyleMaskClosable |
		NSWindowStyleMaskMiniaturizable |
		NSWindowStyleMaskResizable;
	const NSRect content = NSMakeRect(0.0, 0.0, width, height);
	const NSRect frame = [NSWindow frameRectForContentRect:content styleMask:style];
	const NSSize available = screen.visibleFrame.size;
	return frame.size.width <= available.width && frame.size.height <= available.height;
}

struct ResolutionCatalog
{
	std::vector<HP2Launcher::DisplayResolution> windowed;
	std::vector<HP2Launcher::DisplayResolution> fullscreen;
	HP2Launcher::DisplayResolution currentDisplay;
};

ResolutionCatalog EnumerateDisplayResolutions(const HP2Launcher::LauncherRequest& request)
{
	ResolutionCatalog catalog;
	std::set<std::pair<int, int>> windowedSeen;
	std::set<std::pair<int, int>> fullscreenSeen;
	std::set<std::pair<int, int>> nativeModes;
	NSScreen* screen = NSScreen.mainScreen;
	const CGDirectDisplayID display = CGMainDisplayID();
	const std::size_t physicalWidth = CGDisplayPixelsWide(display);
	const std::size_t physicalHeight = CGDisplayPixelsHigh(display);
	bool hasCurrentCGMode = false;

	CGDisplayModeRef currentMode = CGDisplayCopyDisplayMode(display);
	if (currentMode != nullptr)
	{
		const std::size_t width = CGDisplayModeGetWidth(currentMode);
		const std::size_t height = CGDisplayModeGetHeight(currentMode);
		if (IsRepresentableResolution(width, height))
		{
			catalog.currentDisplay.width = static_cast<int>(width);
			catalog.currentDisplay.height = static_cast<int>(height);
			hasCurrentCGMode = true;
		}
		CFRelease(currentMode);
	}
	if (!hasCurrentCGMode && screen != nil)
	{
		const NSSize logicalSize = screen.frame.size;
		if (logicalSize.width >= MinimumResolutionWidth &&
			logicalSize.height >= MinimumResolutionHeight &&
			logicalSize.width <= MaximumResolutionDimension &&
			logicalSize.height <= MaximumResolutionDimension)
		{
			catalog.currentDisplay.width = static_cast<int>(std::floor(logicalSize.width));
			catalog.currentDisplay.height = static_cast<int>(std::floor(logicalSize.height));
		}
	}

	CFArrayRef displayModes = CGDisplayCopyAllDisplayModes(display, nullptr);
	if (displayModes != nullptr)
	{
		const CFIndex modeCount = CFArrayGetCount(displayModes);
		for (CFIndex modeIndex = 0; modeIndex < modeCount; ++modeIndex)
		{
			CGDisplayModeRef mode = static_cast<CGDisplayModeRef>(
				const_cast<void*>(CFArrayGetValueAtIndex(displayModes, modeIndex)));
			if (mode == nullptr)
			{
				continue;
			}
			const std::size_t width = CGDisplayModeGetWidth(mode);
			const std::size_t height = CGDisplayModeGetHeight(mode);
			if (!IsRepresentableResolution(width, height))
			{
				continue;
			}
			const int logicalWidth = static_cast<int>(width);
			const int logicalHeight = static_cast<int>(height);
			AppendResolution(catalog.fullscreen, fullscreenSeen, logicalWidth, logicalHeight);
			if (CGDisplayModeGetPixelWidth(mode) == physicalWidth &&
				CGDisplayModeGetPixelHeight(mode) == physicalHeight)
			{
				nativeModes.emplace(logicalWidth, logicalHeight);
			}
			if (WindowResolutionFitsVisibleFrame(logicalWidth, logicalHeight, screen))
			{
				AppendResolution(catalog.windowed, windowedSeen, logicalWidth, logicalHeight);
			}
		}
		CFRelease(displayModes);
	}

	auto appendWindowedIfUsable = [&](int width, int height, const std::string& label)
	{
		if (WindowResolutionFitsVisibleFrame(width, height, screen))
		{
			AppendResolution(catalog.windowed, windowedSeen, width, height, label);
		}
	};
	appendWindowedIfUsable(
		request.settings.resolution.width,
		request.settings.resolution.height,
		request.settings.resolution.label);
	for (const HP2Launcher::DisplayResolution& resolution : request.displayModes)
	{
		appendWindowedIfUsable(
			resolution.width,
			resolution.height,
			resolution.label);
	}

	static constexpr int StandardResolutions[][2] =
	{
		{640, 480},
		{800, 600},
		{1024, 768},
		{1280, 720},
		{1280, 800},
		{1280, 1024},
		{1366, 768},
		{1440, 900},
		{1600, 900},
		{1680, 1050},
		{1920, 1080},
		{1920, 1200},
		{2560, 1440},
		{3840, 2160}
	};
	for (const auto& standardResolution : StandardResolutions)
	{
		appendWindowedIfUsable(
			standardResolution[0],
			standardResolution[1],
			ResolutionLabel(standardResolution[0], standardResolution[1]));
	}

	if (catalog.windowed.empty() && screen != nil)
	{
		const NSWindowStyleMask style =
			NSWindowStyleMaskTitled |
			NSWindowStyleMaskClosable |
			NSWindowStyleMaskMiniaturizable |
			NSWindowStyleMaskResizable;
		const NSRect maximumContent = [NSWindow
			contentRectForFrameRect:NSMakeRect(0.0, 0.0, screen.visibleFrame.size.width, screen.visibleFrame.size.height)
			styleMask:style];
		AppendResolution(
			catalog.windowed,
			windowedSeen,
			static_cast<int>(std::floor(maximumContent.size.width)),
			static_cast<int>(std::floor(maximumContent.size.height)));
	}

	SortResolutions(catalog.windowed);
	SortResolutions(catalog.fullscreen);
	const std::pair<int, int> currentDimensions(
		catalog.currentDisplay.width,
		catalog.currentDisplay.height);
	catalog.currentDisplay.label =
		ResolutionLabel(catalog.currentDisplay.width, catalog.currentDisplay.height) +
		(nativeModes.find(currentDimensions) != nativeModes.end()
			? " (Current Display, Native)"
			: " (Current Display)");
	for (HP2Launcher::DisplayResolution& resolution : catalog.fullscreen)
	{
		const std::pair<int, int> dimensions(resolution.width, resolution.height);
		const bool current =
			resolution.width == catalog.currentDisplay.width &&
			resolution.height == catalog.currentDisplay.height;

		const bool native = nativeModes.find(dimensions) != nativeModes.end();
		resolution.label = ResolutionLabel(resolution.width, resolution.height);
		if (current && native)
		{
			resolution.label += " (Current, Native)";
		}
		else if (current)
		{
			resolution.label += " (Current)";
		}
		else if (native)
		{
			resolution.label += " (Native)";
		}
	}
	return catalog;
}

template <std::size_t Count>
bool IsAcceptedScale(double value, const std::array<double, Count>& accepted)
{
	return std::isfinite(value) &&
		std::find(accepted.begin(), accepted.end(), value) != accepted.end();
}

bool ValidateSettings(const HP2Launcher::LauncherSettings& settings, std::string& error)
{
	switch (settings.screenMode)
	{
	case HP2Launcher::ScreenMode::Windowed:
	case HP2Launcher::ScreenMode::Fullscreen:
	case HP2Launcher::ScreenMode::BorderlessDesktop:
		break;
	default:
		error = "Choose a valid window mode.";
		return false;
	}

	switch (settings.controlMode)
	{
	case HP2Launcher::ControlMode::Classic:
	case HP2Launcher::ControlMode::Modern:
		break;
	default:
		error = "Choose a valid control style.";
		return false;
	}

	switch (settings.renderBackend)
	{
	case HP2Launcher::RenderBackend::XOpenGL:
	case HP2Launcher::RenderBackend::Vulkan:
		break;
	default:
		error = "Choose a valid renderer.";
		return false;
	}

	if (settings.resolution.width < MinimumResolutionWidth ||
		settings.resolution.height < MinimumResolutionHeight ||
		settings.resolution.width > MaximumResolutionDimension ||
		settings.resolution.height > MaximumResolutionDimension)
	{
		error = "Choose a resolution between 320 x 320 and 16384 x 16384.";
		return false;
	}

	if (std::find(
			HP2Launcher::FrameRateLimitValues.begin(),
			HP2Launcher::FrameRateLimitValues.end(),
			settings.frameRateLimit) == HP2Launcher::FrameRateLimitValues.end())
	{
		error = "Choose 30, 60, 120, 144, or Unlimited for the frame rate.";
		return false;
	}
	if (std::find(
			HP2Launcher::AntiAliasingSampleValues.begin(),
			HP2Launcher::AntiAliasingSampleValues.end(),
			settings.antiAliasingSamples) == HP2Launcher::AntiAliasingSampleValues.end())
	{
		error = "Choose Off, 2x MSAA, or 4x MSAA for anti-aliasing.";
		return false;
	}
	if (std::find(
			HP2Launcher::AnisotropyValues.begin(),
			HP2Launcher::AnisotropyValues.end(),
			settings.anisotropy) == HP2Launcher::AnisotropyValues.end())
	{
		error = "Choose Off, 4x, 8x, or 16x for texture filtering.";
		return false;
	}

	if (!std::isfinite(settings.brightness) ||
		settings.brightness < MinimumBrightness ||
		settings.brightness > MaximumBrightness)
	{
		error = "Brightness must be between 10% and 100%.";
		return false;
	}
	if (!IsAcceptedScale(settings.renderScale, HP2Launcher::RenderScaleValues))
	{
		error = "Choose an available render scale.";
		return false;
	}
	if (!IsAcceptedScale(settings.uiScale, HP2Launcher::UIScaleValues))
	{
		error = "Choose an available UI scale.";
		return false;
	}


	if (!std::isfinite(settings.soundVolume) ||
		settings.soundVolume < 0.0 || settings.soundVolume > 1.0 ||
		!std::isfinite(settings.musicVolume) ||
		settings.musicVolume < 0.0 || settings.musicVolume > 1.0)
	{
		error = "Sound and music volume must be between 0% and 100%.";
		return false;
	}

	if (!std::isfinite(settings.mouseSensitivity) ||
		settings.mouseSensitivity < MinimumMouseSensitivity ||
		settings.mouseSensitivity > MaximumMouseSensitivity)
	{
		error = "Mouse sensitivity must be between 0.2 and 10.0.";
		return false;
	}

	switch (settings.textureDetail)
	{
	case HP2Launcher::TextureDetail::Low:
	case HP2Launcher::TextureDetail::Medium:
	case HP2Launcher::TextureDetail::High:
		break;
	default:
		error = "Choose a valid texture detail level.";
		return false;
	}

	switch (settings.objectDetail)
	{
	case HP2Launcher::ObjectDetail::VeryLow:
	case HP2Launcher::ObjectDetail::Low:
	case HP2Launcher::ObjectDetail::Medium:
	case HP2Launcher::ObjectDetail::High:
	case HP2Launcher::ObjectDetail::VeryHigh:
		break;
	default:
		error = "Choose a valid object detail level.";
		return false;
	}

	switch (settings.difficulty)
	{
	case HP2Launcher::Difficulty::Easy:
	case HP2Launcher::Difficulty::Medium:
	case HP2Launcher::Difficulty::Hard:
		break;
	default:
		error = "Choose a valid difficulty.";
		return false;
	}

	error.clear();
	return true;
}

void AddTaggedPopupItem(NSPopUpButton* popup, NSString* title, NSInteger tag)
{
	[popup addItemWithTitle:title];
	popup.lastItem.tag = tag;
}

bool SelectPopupItemWithTag(NSPopUpButton* popup, NSInteger tag)
{
	for (NSMenuItem* item in popup.itemArray)
	{
		if (item.tag == tag)
		{
			[popup selectItem:item];
			return true;
		}
	}
	return false;
}

bool IsKnownDataSource(HP2Launcher::DataSource source)
{
	switch (source)
	{
	case HP2Launcher::DataSource::Retail:
	case HP2Launcher::DataSource::Prototype:
		return true;
	default:
		return false;
	}
}

bool CopyCocoaString(NSString* value, std::string& result)
{
	NSData* data = [value dataUsingEncoding:NSUTF8StringEncoding];
	if (data == nil)
	{
		return false;
	}
	result.assign(static_cast<const char*>(data.bytes), data.length);
	return true;
}
}

@interface HP2MacLauncherController : NSObject <NSWindowDelegate, NSTabViewDelegate>
{
	const HP2Launcher::LauncherRequest* _request;
	NSApplication* _application;
	std::vector<HP2Launcher::DisplayResolution> _windowedResolutions;
	std::vector<HP2Launcher::DisplayResolution> _fullscreenResolutions;
	std::vector<HP2Launcher::DisplayResolution> _resolutions;
	HP2Launcher::DisplayResolution _currentDisplayResolution;
	HP2Launcher::DisplayResolution _windowedSelection;
	HP2Launcher::DisplayResolution _fullscreenSelection;
	HP2Launcher::ScreenMode _displayedScreenMode;
	HP2Launcher::DataSourceConfiguration _dataSources;
	HP2Launcher::LauncherResult _result;
	BOOL _completed;

	NSWindow* _window;
	NSDateFormatter* _dateFormatter;
	NSPopUpButton* _dataSourcePopup;
	NSTextField* _dataSourceRootField;
	NSTextField* _dataSourceStatusLabel;
	NSButton* _chooseDataFolderButton;
	NSPopUpButton* _savePopup;
	NSImageView* _thumbnailImageView;
	NSTextField* _saveSummaryLabel;
	NSButton* _continueButton;
	NSButton* _newGameButton;
	NSButton* _quitButton;
	NSButton* _openUserFolderButton;
	NSButton* _revealLogButton;
	NSTabView* _tabView;

	NSPopUpButton* _rendererPopup;
	NSPopUpButton* _screenModePopup;
	NSPopUpButton* _resolutionPopup;
	NSPopUpButton* _renderScalePopup;
	NSPopUpButton* _uiScalePopup;
	NSButton* _verticalSyncCheckbox;
	NSPopUpButton* _frameRatePopup;
	NSButton* _showFPSCheckbox;
	NSPopUpButton* _widescreenViewPopup;
	NSPopUpButton* _textRenderingPopup;
	NSPopUpButton* _antiAliasingPopup;
	NSPopUpButton* _anisotropyPopup;
	NSSlider* _brightnessSlider;
	NSTextField* _brightnessValueLabel;
	NSPopUpButton* _textureDetailPopup;
	NSPopUpButton* _objectDetailPopup;

	NSButton* _soundEnabledCheckbox;
	NSSlider* _soundVolumeSlider;
	NSTextField* _soundVolumeValueLabel;
	NSSlider* _musicVolumeSlider;
	NSTextField* _musicVolumeValueLabel;

	NSSlider* _mouseSensitivitySlider;
	NSTextField* _mouseSensitivityValueLabel;
	NSButton* _invertMouseCheckbox;
	NSPopUpButton* _controlModePopup;
	NSButton* _joystickCheckbox;
	NSButton* _autoCenterCameraCheckbox;
	NSButton* _moveWhileCastingCheckbox;
	NSButton* _autoQuaffCheckbox;
	NSButton* _screenFlashesCheckbox;
	NSPopUpButton* _difficultyPopup;

	NSArray<NSView*>* _mainKeyViews;
	NSArray<NSView*>* _videoKeyViews;
	NSArray<NSView*>* _audioKeyViews;
	NSArray<NSView*>* _gameplayKeyViews;
}
- (instancetype)initWithRequest:(const HP2Launcher::LauncherRequest*)request
	application:(NSApplication*)application;
- (NSWindow*)buildWindow;
- (NSView*)buildGameDataSection;
- (void)showInitialErrorIfNeeded;
- (void)copyResultTo:(HP2Launcher::LauncherResult*)result;
- (void)finalizeAsQuitIfNeeded;
- (void)detach;
- (IBAction)continueGame:(id)sender;
- (IBAction)newGame:(id)sender;
- (IBAction)quit:(id)sender;
- (IBAction)openUserFolder:(id)sender;
- (IBAction)revealLog:(id)sender;
- (IBAction)dataSourceChanged:(id)sender;
- (IBAction)chooseDataFolder:(id)sender;
- (IBAction)saveSelectionChanged:(id)sender;
- (IBAction)displayModeChanged:(id)sender;
- (void)rebuildResolutionPopupForMode:(HP2Launcher::ScreenMode)mode;
- (IBAction)controlModeChanged:(id)sender;
- (IBAction)soundEnabledChanged:(id)sender;
- (IBAction)sliderChanged:(id)sender;
- (HP2Launcher::DataSource)selectedDataSource;
- (std::string)rootForDataSource:(HP2Launcher::DataSource)source;
- (const HP2Launcher::DataSourceOption*)dataSourceOptionForSource:(HP2Launcher::DataSource)source;
- (void)updateDataSourceControls;
- (HP2Launcher::DataSourceConfiguration)dataSourcesFromControls;
@end

@implementation HP2MacLauncherController

- (instancetype)initWithRequest:(const HP2Launcher::LauncherRequest*)request
	application:(NSApplication*)application
{
	self = [super init];
	if (self != nil)
	{
		_request = request;
		_application = application;
		const ResolutionCatalog catalog = EnumerateDisplayResolutions(*request);
		_windowedResolutions = catalog.windowed;
		_fullscreenResolutions = catalog.fullscreen;
		_currentDisplayResolution = catalog.currentDisplay;
		_windowedSelection = request->settings.resolution;
		_fullscreenSelection = request->settings.resolution;
		_displayedScreenMode = static_cast<HP2Launcher::ScreenMode>(-1);
		_result.settings = request->settings;
		_dataSources = request->dataSources;
		_result.selection.action = HP2Launcher::LaunchAction::Quit;
		_completed = NO;

		_dateFormatter = [[NSDateFormatter alloc] init];
		_dateFormatter.dateStyle = NSDateFormatterMediumStyle;
		_dateFormatter.timeStyle = NSDateFormatterShortStyle;
		_dateFormatter.doesRelativeDateFormatting = YES;
	}
	return self;
}

- (NSTextField*)staticLabel:(NSString*)text
{
	NSTextField* label = [NSTextField labelWithString:text];
	label.translatesAutoresizingMaskIntoConstraints = NO;
	return label;
}

- (NSTextField*)valueLabel
{
	NSTextField* label = [self staticLabel:@""];
	label.alignment = NSTextAlignmentRight;
	label.textColor = NSColor.secondaryLabelColor;
	[label.widthAnchor constraintEqualToConstant:ValueLabelWidth].active = YES;
	return label;
}

- (NSButton*)checkboxWithTitle:(NSString*)title action:(SEL)action
{
	NSButton* checkbox = [[NSButton alloc] initWithFrame:NSZeroRect];
	checkbox.translatesAutoresizingMaskIntoConstraints = NO;
	checkbox.buttonType = NSButtonTypeSwitch;
	checkbox.title = title;
	checkbox.target = self;
	checkbox.action = action;
	[checkbox setAccessibilityLabel:title];
	return checkbox;
}

- (NSButton*)actionButtonWithTitle:(NSString*)title action:(SEL)action
{
	NSButton* button = [[NSButton alloc] initWithFrame:NSZeroRect];
	button.translatesAutoresizingMaskIntoConstraints = NO;
	button.bezelStyle = NSBezelStyleRounded;
	button.title = title;
	button.target = self;
	button.action = action;
	[button setAccessibilityLabel:title];
	[button.widthAnchor constraintEqualToConstant:ActionButtonWidth].active = YES;
	return button;
}
- (NSButton*)utilityButtonWithTitle:(NSString*)title action:(SEL)action
{
	NSButton* button = [[NSButton alloc] initWithFrame:NSZeroRect];
	button.translatesAutoresizingMaskIntoConstraints = NO;
	button.bezelStyle = NSBezelStyleRounded;
	button.controlSize = NSControlSizeSmall;
	button.font = [NSFont systemFontOfSize:NSFont.smallSystemFontSize];
	button.title = title;
	button.target = self;
	button.action = action;
	[button setAccessibilityLabel:title];
	return button;
}


- (NSPopUpButton*)popupWithAccessibilityLabel:(NSString*)label
{
	NSPopUpButton* popup = [[NSPopUpButton alloc] initWithFrame:NSZeroRect pullsDown:NO];
	popup.translatesAutoresizingMaskIntoConstraints = NO;
	[popup setAccessibilityLabel:label];
	[popup.widthAnchor constraintGreaterThanOrEqualToConstant:PopupMinimumWidth].active = YES;
	return popup;
}

- (NSSlider*)sliderWithMinimum:(double)minimum
	maximum:(double)maximum
	value:(double)value
	accessibilityLabel:(NSString*)label
{
	NSSlider* slider = [[NSSlider alloc] initWithFrame:NSZeroRect];
	slider.translatesAutoresizingMaskIntoConstraints = NO;
	slider.minValue = minimum;
	slider.maxValue = maximum;
	slider.doubleValue = std::clamp(value, minimum, maximum);
	slider.continuous = YES;
	slider.target = self;
	slider.action = @selector(sliderChanged:);
	[slider setAccessibilityLabel:label];
	[slider.widthAnchor constraintGreaterThanOrEqualToConstant:SliderMinimumWidth].active = YES;
	return slider;
}

- (NSStackView*)horizontalStackWithViews:(NSArray<NSView*>*)views spacing:(CGFloat)spacing
{
	NSStackView* stack = [NSStackView stackViewWithViews:views];
	stack.translatesAutoresizingMaskIntoConstraints = NO;
	stack.orientation = NSUserInterfaceLayoutOrientationHorizontal;
	stack.alignment = NSLayoutAttributeCenterY;
	stack.spacing = spacing;
	return stack;
}

- (NSStackView*)sliderControlWithSlider:(NSSlider*)slider
	valueLabel:(NSTextField*)valueLabel
	minimumText:(NSString*)minimumText
	maximumText:(NSString*)maximumText
{
	NSTextField* minimumLabel = [self staticLabel:minimumText];
	minimumLabel.font = [NSFont systemFontOfSize:NSFont.smallSystemFontSize];
	minimumLabel.textColor = NSColor.secondaryLabelColor;
	[minimumLabel setAccessibilityLabel:[NSString stringWithFormat:@"Minimum %@", minimumText]];

	NSTextField* maximumLabel = [self staticLabel:maximumText];
	maximumLabel.font = [NSFont systemFontOfSize:NSFont.smallSystemFontSize];
	maximumLabel.textColor = NSColor.secondaryLabelColor;
	[maximumLabel setAccessibilityLabel:[NSString stringWithFormat:@"Maximum %@", maximumText]];

	return [self horizontalStackWithViews:@[
		minimumLabel,
		slider,
		maximumLabel,
		valueLabel
	] spacing:ItemSpacing];
}

- (NSStackView*)verticalStackWithViews:(NSArray<NSView*>*)views spacing:(CGFloat)spacing
{
	NSStackView* stack = [NSStackView stackViewWithViews:views];
	stack.translatesAutoresizingMaskIntoConstraints = NO;
	stack.orientation = NSUserInterfaceLayoutOrientationVertical;
	stack.alignment = NSLayoutAttributeLeading;
	stack.spacing = spacing;
	return stack;
}

- (NSView*)formRowWithTitle:(NSString*)title control:(NSView*)control
{
	NSTextField* label = [self staticLabel:title];
	label.alignment = NSTextAlignmentRight;
	[label.widthAnchor constraintEqualToConstant:FormLabelWidth].active = YES;

	NSStackView* row = [self horizontalStackWithViews:@[label, control] spacing:RowSpacing];
	[row setHuggingPriority:NSLayoutPriorityDefaultLow
		forOrientation:NSLayoutConstraintOrientationHorizontal];
	return row;
}

- (NSScrollView*)scrollViewForForm:(NSStackView*)form
{
	NSScrollView* scrollView = [[NSScrollView alloc] initWithFrame:NSZeroRect];
	scrollView.translatesAutoresizingMaskIntoConstraints = YES;
	scrollView.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
	scrollView.borderType = NSNoBorder;
	scrollView.drawsBackground = NO;
	scrollView.hasVerticalScroller = YES;
	scrollView.hasHorizontalScroller = NO;
	scrollView.autohidesScrollers = YES;

	NSView* documentView = [[NSView alloc] initWithFrame:NSZeroRect];
	documentView.translatesAutoresizingMaskIntoConstraints = NO;
	scrollView.documentView = documentView;
	[documentView addSubview:form];

	NSClipView* clipView = scrollView.contentView;
	[NSLayoutConstraint activateConstraints:@[
		[documentView.leadingAnchor constraintEqualToAnchor:clipView.leadingAnchor],
		[documentView.topAnchor constraintEqualToAnchor:clipView.topAnchor],
		[documentView.widthAnchor constraintEqualToAnchor:clipView.widthAnchor],
		[documentView.heightAnchor constraintGreaterThanOrEqualToAnchor:clipView.heightAnchor],
		[form.leadingAnchor constraintEqualToAnchor:documentView.leadingAnchor constant:TabContentInset],
		[form.trailingAnchor constraintEqualToAnchor:documentView.trailingAnchor constant:-TabContentInset],
		[form.topAnchor constraintEqualToAnchor:documentView.topAnchor constant:TabContentInset],
		[form.bottomAnchor constraintEqualToAnchor:documentView.bottomAnchor constant:-TabContentInset]
	]];

	return scrollView;
}

- (NSString*)savePopupTitleForRecord:(const HP2Launcher::SaveRecord&)save
{
	NSString* name = CocoaString(save.displayName);
	NSString* location = nil;
	if (save.usesSlotDirectory)
	{
		location = save.slot >= 0
			? [NSString stringWithFormat:@"Slot %d", save.slot]
			: @"Slot save";
	}
	else
	{
		location = @"Flat save";
	}

	NSString* dateText = @"Date unavailable";
	if (save.modifiedSeconds > 0)
	{
		NSDate* date = [NSDate dateWithTimeIntervalSince1970:
			static_cast<NSTimeInterval>(save.modifiedSeconds)];
		NSString* formattedDate = [_dateFormatter stringFromDate:date];
		if (formattedDate.length > 0)
		{
			dateText = formattedDate;
		}
	}

	if (name.length == 0)
	{
		return [NSString stringWithFormat:@"%@ — %@", location, dateText];
	}
	return [NSString stringWithFormat:@"%@ — %@ — %@", name, location, dateText];
}

- (NSView*)buildGameDataSection
{
	NSTextField* gameDataLabel = [self staticLabel:@"Game Data"];
	gameDataLabel.font = [NSFont boldSystemFontOfSize:NSFont.systemFontSize];

	_dataSourcePopup = [self popupWithAccessibilityLabel:@"Game data source"];
	AddTaggedPopupItem(
		_dataSourcePopup,
		@"Retail",
		static_cast<NSInteger>(HP2Launcher::DataSource::Retail));
	AddTaggedPopupItem(
		_dataSourcePopup,
		@"Prototype / Beta",
		static_cast<NSInteger>(HP2Launcher::DataSource::Prototype));
	const HP2Launcher::DataSource selectedSource =
		IsKnownDataSource(_dataSources.selected)
			? _dataSources.selected
			: HP2Launcher::DataSource::Retail;
	SelectPopupItemWithTag(_dataSourcePopup, static_cast<NSInteger>(selectedSource));
	_dataSourcePopup.target = self;
	_dataSourcePopup.action = @selector(dataSourceChanged:);

	_dataSourceRootField = [[NSTextField alloc] initWithFrame:NSZeroRect];
	_dataSourceRootField.translatesAutoresizingMaskIntoConstraints = NO;
	_dataSourceRootField.editable = NO;
	_dataSourceRootField.selectable = YES;
	_dataSourceRootField.bezeled = YES;
	_dataSourceRootField.drawsBackground = YES;
	_dataSourceRootField.backgroundColor = NSColor.controlBackgroundColor;
	_dataSourceRootField.usesSingleLineMode = YES;
	_dataSourceRootField.lineBreakMode = NSLineBreakByTruncatingMiddle;
	[_dataSourceRootField setAccessibilityLabel:@"Game data folder"];
	[_dataSourceRootField.widthAnchor constraintGreaterThanOrEqualToConstant:PopupMinimumWidth].active = YES;
	[_dataSourceRootField setContentCompressionResistancePriority:NSLayoutPriorityDefaultLow
		forOrientation:NSLayoutConstraintOrientationHorizontal];

	_chooseDataFolderButton = [self
		utilityButtonWithTitle:@"Choose Folder…"
		action:@selector(chooseDataFolder:)];
	[_chooseDataFolderButton setAccessibilityLabel:@"Choose game data folder"];

	NSStackView* folderControls = [self horizontalStackWithViews:@[
		_dataSourceRootField,
		_chooseDataFolderButton
	] spacing:ItemSpacing];
	[folderControls setHuggingPriority:NSLayoutPriorityDefaultLow
		forOrientation:NSLayoutConstraintOrientationHorizontal];

	_dataSourceStatusLabel = [self staticLabel:@""];
	_dataSourceStatusLabel.font = [NSFont systemFontOfSize:NSFont.smallSystemFontSize];
	_dataSourceStatusLabel.textColor = NSColor.secondaryLabelColor;
	_dataSourceStatusLabel.lineBreakMode = NSLineBreakByWordWrapping;
	_dataSourceStatusLabel.maximumNumberOfLines = 2;
	[_dataSourceStatusLabel setAccessibilityLabel:@"Game data status"];
	[_dataSourceStatusLabel setContentCompressionResistancePriority:NSLayoutPriorityDefaultLow
		forOrientation:NSLayoutConstraintOrientationHorizontal];

	NSStackView* form = [self verticalStackWithViews:@[
		[self formRowWithTitle:@"Source" control:_dataSourcePopup],
		[self formRowWithTitle:@"Folder" control:folderControls],
		[self formRowWithTitle:@"" control:_dataSourceStatusLabel]
	] spacing:RowSpacing];
	[form setHuggingPriority:NSLayoutPriorityDefaultLow
		forOrientation:NSLayoutConstraintOrientationHorizontal];

	NSStackView* section = [self verticalStackWithViews:@[
		gameDataLabel,
		form
	] spacing:ItemSpacing];
	[section setHuggingPriority:NSLayoutPriorityDefaultLow
		forOrientation:NSLayoutConstraintOrientationHorizontal];
	[self updateDataSourceControls];
	return section;
}

- (NSView*)buildSaveSection
{
	_thumbnailImageView = [[NSImageView alloc] initWithFrame:NSZeroRect];
	_thumbnailImageView.translatesAutoresizingMaskIntoConstraints = NO;
	_thumbnailImageView.imageAlignment = NSImageAlignCenter;
	_thumbnailImageView.imageFrameStyle = NSImageFrameGrayBezel;
	_thumbnailImageView.imageScaling = NSImageScaleProportionallyUpOrDown;
	[_thumbnailImageView.widthAnchor constraintEqualToConstant:ThumbnailWidth].active = YES;
	[_thumbnailImageView.heightAnchor constraintEqualToConstant:ThumbnailHeight].active = YES;
	[_thumbnailImageView setAccessibilityLabel:@"Selected saved game thumbnail"];

	NSTextField* saveLabel = [self staticLabel:@"Saved Game"];
	saveLabel.font = [NSFont boldSystemFontOfSize:NSFont.systemFontSize];

	_savePopup = [self popupWithAccessibilityLabel:@"Saved game"];
	_savePopup.target = self;
	_savePopup.action = @selector(saveSelectionChanged:);
	if (_request->saves.empty())
	{
		[_savePopup addItemWithTitle:@"No saved games found"];
		_savePopup.enabled = NO;
	}
	else
	{
		for (const HP2Launcher::SaveRecord& save : _request->saves)
		{
			[_savePopup addItemWithTitle:[self savePopupTitleForRecord:save]];
		}
		[_savePopup selectItemAtIndex:0];
	}

	_saveSummaryLabel = [self staticLabel:_request->saves.empty()
		? @"Start a new game to begin your adventure."
		: @"Saved games are opened without being changed here."];
	_saveSummaryLabel.textColor = NSColor.secondaryLabelColor;
	_saveSummaryLabel.font = [NSFont systemFontOfSize:NSFont.smallSystemFontSize];
	_saveSummaryLabel.lineBreakMode = NSLineBreakByWordWrapping;
	_saveSummaryLabel.maximumNumberOfLines = 2;
	[_saveSummaryLabel setContentCompressionResistancePriority:NSLayoutPriorityDefaultLow
		forOrientation:NSLayoutConstraintOrientationHorizontal];

	NSStackView* saveControls = [self verticalStackWithViews:@[
		saveLabel,
		_savePopup,
		_saveSummaryLabel
	] spacing:ItemSpacing];
	[saveControls setHuggingPriority:NSLayoutPriorityDefaultLow
		forOrientation:NSLayoutConstraintOrientationHorizontal];

	_continueButton = [self actionButtonWithTitle:@"Continue" action:@selector(continueGame:)];
	_continueButton.enabled = !_request->saves.empty();
	_newGameButton = [self actionButtonWithTitle:@"New Game" action:@selector(newGame:)];
	_quitButton = [self actionButtonWithTitle:@"Quit" action:@selector(quit:)];
	_quitButton.keyEquivalent = @"\x1b";
	_quitButton.keyEquivalentModifierMask = 0;

	NSStackView* actionButtons = [self verticalStackWithViews:@[
		_continueButton,
		_newGameButton,
		_quitButton
	] spacing:ItemSpacing];
	actionButtons.alignment = NSLayoutAttributeTrailing;

	NSStackView* saveSection = [self horizontalStackWithViews:@[
		_thumbnailImageView,
		saveControls,
		actionButtons
	] spacing:SectionSpacing];
	[saveControls.widthAnchor constraintGreaterThanOrEqualToConstant:PopupMinimumWidth].active = YES;

	[self updateSelectedSavePreview];
	return saveSection;
}

- (NSView*)buildVideoTab
{
	_rendererPopup = [self popupWithAccessibilityLabel:@"Renderer"];
	AddTaggedPopupItem(_rendererPopup, @"XOpenGL", static_cast<NSInteger>(HP2Launcher::RenderBackend::XOpenGL));
	AddTaggedPopupItem(_rendererPopup, @"Vulkan (Experimental)", static_cast<NSInteger>(HP2Launcher::RenderBackend::Vulkan));
	_rendererPopup.menu.autoenablesItems = NO;
	for (NSMenuItem* item in _rendererPopup.itemArray)
		item.enabled = YES;
#if !HP2_ENABLE_VULKAN_DRIVER
	_rendererPopup.itemArray[1].enabled = NO;
#endif
	if (!SelectPopupItemWithTag(_rendererPopup, static_cast<NSInteger>(_request->settings.renderBackend)) ||
		!_rendererPopup.selectedItem.enabled)
	{
		SelectPopupItemWithTag(_rendererPopup, static_cast<NSInteger>(HP2Launcher::RenderBackend::XOpenGL));
	}
	[_rendererPopup setAccessibilityHelp:
		@"Chooses the 3D renderer. XOpenGL is the default; Vulkan is experimental and may not be "
		@"included in this build."];

	_screenModePopup = [self popupWithAccessibilityLabel:@"Window mode"];
	AddTaggedPopupItem(_screenModePopup, @"Windowed", static_cast<NSInteger>(HP2Launcher::ScreenMode::Windowed));
	AddTaggedPopupItem(_screenModePopup, @"Fullscreen (Selected Resolution)", static_cast<NSInteger>(HP2Launcher::ScreenMode::Fullscreen));
	AddTaggedPopupItem(_screenModePopup, @"Borderless Desktop (Current Display)", static_cast<NSInteger>(HP2Launcher::ScreenMode::BorderlessDesktop));
	_screenModePopup.itemArray[1].enabled = !_fullscreenResolutions.empty();
	if (!SelectPopupItemWithTag(_screenModePopup, static_cast<NSInteger>(_request->settings.screenMode)) ||
		!_screenModePopup.selectedItem.enabled)
	{
		[_screenModePopup selectItemAtIndex:0];
	}
	_screenModePopup.target = self;
	_screenModePopup.action = @selector(displayModeChanged:);

	_resolutionPopup = [self popupWithAccessibilityLabel:@"Resolution"];

	_renderScalePopup = [self popupWithAccessibilityLabel:@"Render scale"];
	AddTaggedPopupItem(_renderScalePopup, @"50% — Fastest", 50);
	AddTaggedPopupItem(_renderScalePopup, @"67% — Faster", 67);
	AddTaggedPopupItem(_renderScalePopup, @"75% — Balanced", 75);
	AddTaggedPopupItem(_renderScalePopup, @"85% — Sharper", 85);
	AddTaggedPopupItem(_renderScalePopup, @"100% — Full Quality", 100);
	const NSInteger selectedRenderScaleTag =
		IsAcceptedScale(_request->settings.renderScale, HP2Launcher::RenderScaleValues)
			? static_cast<NSInteger>(std::lround(_request->settings.renderScale * 100.0))
			: 100;
	if (!SelectPopupItemWithTag(_renderScalePopup, selectedRenderScaleTag))
	{
		SelectPopupItemWithTag(_renderScalePopup, 100);
	}
	[_renderScalePopup setAccessibilityHelp:
		@"Controls the 3D scene resolution. Lower values improve performance while the interface stays sharp."];

	_uiScalePopup = [self popupWithAccessibilityLabel:@"UI scale"];
	AddTaggedPopupItem(_uiScalePopup, @"75% — Compact", 75);
	AddTaggedPopupItem(_uiScalePopup, @"100% — Standard", 100);
	AddTaggedPopupItem(_uiScalePopup, @"125% — Larger", 125);
	AddTaggedPopupItem(_uiScalePopup, @"150% — Large", 150);
	AddTaggedPopupItem(_uiScalePopup, @"175% — Extra Large", 175);
	AddTaggedPopupItem(_uiScalePopup, @"200% — Largest", 200);
	const NSInteger selectedUIScaleTag =
		IsAcceptedScale(_request->settings.uiScale, HP2Launcher::UIScaleValues)
			? static_cast<NSInteger>(std::lround(_request->settings.uiScale * 100.0))
			: 100;
	if (!SelectPopupItemWithTag(_uiScalePopup, selectedUIScaleTag))
	{
		SelectPopupItemWithTag(_uiScalePopup, 100);
	}
	[_uiScalePopup setAccessibilityHelp:
		@"Controls in-game text and interface size. Higher values are easier to read."];
	_textRenderingPopup = [self popupWithAccessibilityLabel:@"Text rendering"];
	AddTaggedPopupItem(_textRenderingPopup, @"Clear Native Text (CoreText)", 1);
	AddTaggedPopupItem(_textRenderingPopup, @"Original Bitmap Fonts", 0);
	_textRenderingPopup.menu.autoenablesItems = NO;
	for (NSMenuItem* item in _textRenderingPopup.itemArray)
		item.enabled = YES;
	_textRenderingPopup.enabled = YES;
	SelectPopupItemWithTag(_textRenderingPopup, _request->settings.nativeText ? 1 : 0);
	[_textRenderingPopup setAccessibilityHelp:
		@"Clear Native Text uses CoreText and safely falls back to Original Bitmap Fonts if native drawing fails."];
	_widescreenViewPopup = [self popupWithAccessibilityLabel:@"Widescreen view"];
	AddTaggedPopupItem(_widescreenViewPopup, @"Preserve Vertical View", 1);
	AddTaggedPopupItem(_widescreenViewPopup, @"Original 4:3 Framing", 0);
	SelectPopupItemWithTag(_widescreenViewPopup, _request->settings.maintainVerticalFOV ? 1 : 0);
	[_widescreenViewPopup setAccessibilityHelp:
		@"Preserve Vertical View expands the horizontal view on widescreen displays without cropping the scene."];


	_verticalSyncCheckbox = [self checkboxWithTitle:@"Vertical Sync" action:nil];
	_verticalSyncCheckbox.state = _request->settings.verticalSync
		? NSControlStateValueOn : NSControlStateValueOff;
	[_verticalSyncCheckbox setAccessibilityHelp:
		@"Synchronizes completed frames with the display to prevent tearing."];

	_frameRatePopup = [self popupWithAccessibilityLabel:@"Frame rate limit"];
	AddTaggedPopupItem(_frameRatePopup, @"30 FPS", 30);
	AddTaggedPopupItem(_frameRatePopup, @"60 FPS", 60);
	AddTaggedPopupItem(_frameRatePopup, @"120 FPS — High Refresh", 120);
	AddTaggedPopupItem(_frameRatePopup, @"144 FPS — High Refresh", 144);
	AddTaggedPopupItem(_frameRatePopup, @"Unlimited", 0);
	if (!SelectPopupItemWithTag(_frameRatePopup, _request->settings.frameRateLimit))
	{
		SelectPopupItemWithTag(_frameRatePopup, 60);
	}
	_showFPSCheckbox = [self checkboxWithTitle:@"Show FPS" action:nil];
	_showFPSCheckbox.state = _request->settings.showFPS
		? NSControlStateValueOn : NSControlStateValueOff;
	[_showFPSCheckbox setAccessibilityHelp:
		@"Shows the current frame rate as a small number in the top-right corner."];
	_antiAliasingPopup = [self popupWithAccessibilityLabel:@"Anti-aliasing"];
	AddTaggedPopupItem(_antiAliasingPopup, @"Off", 0);
	AddTaggedPopupItem(_antiAliasingPopup, @"2× MSAA", 2);
	AddTaggedPopupItem(_antiAliasingPopup, @"4× MSAA", 4);
	if (!SelectPopupItemWithTag(_antiAliasingPopup, _request->settings.antiAliasingSamples))
	{
		SelectPopupItemWithTag(_antiAliasingPopup, 0);
	}
	[_antiAliasingPopup setAccessibilityHelp:
		@"Uses multisample anti-aliasing to smooth 3D scene edges."];

	_anisotropyPopup = [self popupWithAccessibilityLabel:@"Texture filtering"];
	AddTaggedPopupItem(_anisotropyPopup, @"Off", 0);
	AddTaggedPopupItem(_anisotropyPopup, @"4×", 4);
	AddTaggedPopupItem(_anisotropyPopup, @"8×", 8);
	AddTaggedPopupItem(_anisotropyPopup, @"16×", 16);
	if (!SelectPopupItemWithTag(_anisotropyPopup, _request->settings.anisotropy))
	{
		SelectPopupItemWithTag(_anisotropyPopup, 4);
	}
	[_anisotropyPopup setAccessibilityHelp:
		@"Improves the clarity of textures viewed at an angle."];


	_brightnessSlider = [self
		sliderWithMinimum:MinimumBrightness
		maximum:MaximumBrightness
		value:_request->settings.brightness
		accessibilityLabel:@"Brightness"];
	_brightnessValueLabel = [self valueLabel];
	NSStackView* brightnessControl = [self
		sliderControlWithSlider:_brightnessSlider
		valueLabel:_brightnessValueLabel
		minimumText:@"10%"
		maximumText:@"100%"];

	_textureDetailPopup = [self popupWithAccessibilityLabel:@"Texture detail"];
	AddTaggedPopupItem(_textureDetailPopup, @"Low", static_cast<NSInteger>(HP2Launcher::TextureDetail::Low));
	AddTaggedPopupItem(_textureDetailPopup, @"Medium", static_cast<NSInteger>(HP2Launcher::TextureDetail::Medium));
	AddTaggedPopupItem(_textureDetailPopup, @"High", static_cast<NSInteger>(HP2Launcher::TextureDetail::High));
	if (!SelectPopupItemWithTag(_textureDetailPopup, static_cast<NSInteger>(_request->settings.textureDetail)))
	{
		SelectPopupItemWithTag(_textureDetailPopup, static_cast<NSInteger>(HP2Launcher::TextureDetail::High));
	}

	_objectDetailPopup = [self popupWithAccessibilityLabel:@"Object detail"];
	AddTaggedPopupItem(_objectDetailPopup, @"Very Low", static_cast<NSInteger>(HP2Launcher::ObjectDetail::VeryLow));
	AddTaggedPopupItem(_objectDetailPopup, @"Low", static_cast<NSInteger>(HP2Launcher::ObjectDetail::Low));
	AddTaggedPopupItem(_objectDetailPopup, @"Medium", static_cast<NSInteger>(HP2Launcher::ObjectDetail::Medium));
	AddTaggedPopupItem(_objectDetailPopup, @"High", static_cast<NSInteger>(HP2Launcher::ObjectDetail::High));
	AddTaggedPopupItem(_objectDetailPopup, @"Very High", static_cast<NSInteger>(HP2Launcher::ObjectDetail::VeryHigh));
	if (!SelectPopupItemWithTag(_objectDetailPopup, static_cast<NSInteger>(_request->settings.objectDetail)))
	{
		SelectPopupItemWithTag(_objectDetailPopup, static_cast<NSInteger>(HP2Launcher::ObjectDetail::Medium));
	}

	NSStackView* form = [self verticalStackWithViews:@[
		[self formRowWithTitle:@"Renderer" control:_rendererPopup],
		[self formRowWithTitle:@"Window Mode" control:_screenModePopup],
		[self formRowWithTitle:@"Resolution" control:_resolutionPopup],
		[self formRowWithTitle:@"Render Scale" control:_renderScalePopup],
		[self formRowWithTitle:@"UI Scale" control:_uiScalePopup],
		[self formRowWithTitle:@"Text Rendering" control:_textRenderingPopup],
		[self formRowWithTitle:@"Widescreen View" control:_widescreenViewPopup],
		[self formRowWithTitle:@"" control:_verticalSyncCheckbox],
		[self formRowWithTitle:@"Frame Rate" control:_frameRatePopup],
		[self formRowWithTitle:@"" control:_showFPSCheckbox],
		[self formRowWithTitle:@"Anti-Aliasing" control:_antiAliasingPopup],
		[self formRowWithTitle:@"Texture Filtering" control:_anisotropyPopup],
		[self formRowWithTitle:@"Brightness" control:brightnessControl],
		[self formRowWithTitle:@"Texture Detail" control:_textureDetailPopup],
		[self formRowWithTitle:@"Object Detail" control:_objectDetailPopup]
	] spacing:RowSpacing];

	_videoKeyViews = @[
		_rendererPopup,
		_screenModePopup,
		_resolutionPopup,
		_renderScalePopup,
		_uiScalePopup,
		_textRenderingPopup,
		_widescreenViewPopup,
		_verticalSyncCheckbox,
		_frameRatePopup,
		_showFPSCheckbox,
		_antiAliasingPopup,
		_anisotropyPopup,
		_brightnessSlider,
		_textureDetailPopup,
		_objectDetailPopup
	];
	[self displayModeChanged:nil];
	return [self scrollViewForForm:form];
}

- (NSView*)buildAudioTab
{
	_soundEnabledCheckbox = [self checkboxWithTitle:@"Enable Sound" action:@selector(soundEnabledChanged:)];
	_soundEnabledCheckbox.state = _request->settings.soundEnabled
		? NSControlStateValueOn : NSControlStateValueOff;

	_soundVolumeSlider = [self
		sliderWithMinimum:0.0
		maximum:1.0
		value:_request->settings.soundVolume
		accessibilityLabel:@"Sound volume"];
	_soundVolumeValueLabel = [self valueLabel];
	NSStackView* soundVolumeControl = [self
		sliderControlWithSlider:_soundVolumeSlider
		valueLabel:_soundVolumeValueLabel
		minimumText:@"0%"
		maximumText:@"100%"];

	_musicVolumeSlider = [self
		sliderWithMinimum:0.0
		maximum:1.0
		value:_request->settings.musicVolume
		accessibilityLabel:@"Music volume"];
	_musicVolumeValueLabel = [self valueLabel];
	NSStackView* musicVolumeControl = [self
		sliderControlWithSlider:_musicVolumeSlider
		valueLabel:_musicVolumeValueLabel
		minimumText:@"0%"
		maximumText:@"100%"];

	NSStackView* form = [self verticalStackWithViews:@[
		[self formRowWithTitle:@"" control:_soundEnabledCheckbox],
		[self formRowWithTitle:@"Sound Volume" control:soundVolumeControl],
		[self formRowWithTitle:@"Music Volume" control:musicVolumeControl]
	] spacing:RowSpacing];

	_audioKeyViews = @[
		_soundEnabledCheckbox,
		_soundVolumeSlider,
		_musicVolumeSlider
	];
	[self soundEnabledChanged:nil];
	return [self scrollViewForForm:form];
}

- (NSView*)buildControlsAndGameplayTab
{
	_controlModePopup = [self popupWithAccessibilityLabel:@"Control style"];
	AddTaggedPopupItem(_controlModePopup, @"Classic", static_cast<NSInteger>(HP2Launcher::ControlMode::Classic));
	_controlModePopup.target = self;
	_controlModePopup.action = @selector(controlModeChanged:);
	AddTaggedPopupItem(_controlModePopup, @"Modern", static_cast<NSInteger>(HP2Launcher::ControlMode::Modern));
	if (!SelectPopupItemWithTag(_controlModePopup, static_cast<NSInteger>(_request->settings.controlMode)))
	{
		SelectPopupItemWithTag(_controlModePopup, static_cast<NSInteger>(HP2Launcher::ControlMode::Classic));
	}
	[_controlModePopup setAccessibilityHelp:
		@"Modern keeps the camera in free orbit without movement recentering, uses camera-relative movement, and maps the right stick to the camera without changing top speed."];

	_mouseSensitivitySlider = [self
		sliderWithMinimum:MinimumMouseSensitivity
		maximum:MaximumMouseSensitivity
		value:_request->settings.mouseSensitivity
		accessibilityLabel:@"Mouse sensitivity"];
	_mouseSensitivityValueLabel = [self valueLabel];
	NSStackView* mouseSensitivityControl = [self
		sliderControlWithSlider:_mouseSensitivitySlider
		valueLabel:_mouseSensitivityValueLabel
		minimumText:@"0.2"
		maximumText:@"10.0"];

	_invertMouseCheckbox = [self checkboxWithTitle:@"Invert Mouse" action:nil];
	_invertMouseCheckbox.state = _request->settings.invertMouse
		? NSControlStateValueOn : NSControlStateValueOff;
	_joystickCheckbox = [self checkboxWithTitle:@"Enable Joystick" action:nil];
	_joystickCheckbox.state = _request->settings.joystickEnabled
		? NSControlStateValueOn : NSControlStateValueOff;
	_autoCenterCameraCheckbox = [self checkboxWithTitle:@"Auto-Center Camera" action:nil];
	_autoCenterCameraCheckbox.state = _request->settings.autoCenterCamera
		? NSControlStateValueOn : NSControlStateValueOff;
	[_autoCenterCameraCheckbox setAccessibilityHelp:
		@"Classic can recenter camera pitch while Harry moves. Modern always keeps movement from recentering the camera."];
	_moveWhileCastingCheckbox = [self checkboxWithTitle:@"Move While Casting" action:nil];
	_moveWhileCastingCheckbox.state = _request->settings.moveWhileCasting
		? NSControlStateValueOn : NSControlStateValueOff;
	_autoQuaffCheckbox = [self checkboxWithTitle:@"Auto-Quaff Potions" action:nil];
	_autoQuaffCheckbox.state = _request->settings.autoQuaff
		? NSControlStateValueOn : NSControlStateValueOff;
	_screenFlashesCheckbox = [self checkboxWithTitle:@"Screen Flashes" action:nil];
	_screenFlashesCheckbox.state = _request->settings.screenFlashes
		? NSControlStateValueOn : NSControlStateValueOff;
	[_screenFlashesCheckbox setAccessibilityHelp:
		@"Turn this off to reduce full-screen flashes and abrupt brightness changes."];


	_difficultyPopup = [self popupWithAccessibilityLabel:@"Difficulty"];
	AddTaggedPopupItem(_difficultyPopup, @"Easy", static_cast<NSInteger>(HP2Launcher::Difficulty::Easy));
	AddTaggedPopupItem(_difficultyPopup, @"Medium", static_cast<NSInteger>(HP2Launcher::Difficulty::Medium));
	AddTaggedPopupItem(_difficultyPopup, @"Hard", static_cast<NSInteger>(HP2Launcher::Difficulty::Hard));
	if (!SelectPopupItemWithTag(_difficultyPopup, static_cast<NSInteger>(_request->settings.difficulty)))
	{
		SelectPopupItemWithTag(_difficultyPopup, static_cast<NSInteger>(HP2Launcher::Difficulty::Easy));
	}

	NSStackView* form = [self verticalStackWithViews:@[
		[self formRowWithTitle:@"Control Style" control:_controlModePopup],
		[self formRowWithTitle:@"Mouse Sensitivity" control:mouseSensitivityControl],
		[self formRowWithTitle:@"" control:_invertMouseCheckbox],
		[self formRowWithTitle:@"" control:_joystickCheckbox],
		[self formRowWithTitle:@"" control:_autoCenterCameraCheckbox],
		[self formRowWithTitle:@"" control:_moveWhileCastingCheckbox],
		[self formRowWithTitle:@"" control:_autoQuaffCheckbox],
		[self formRowWithTitle:@"" control:_screenFlashesCheckbox],
		[self formRowWithTitle:@"Difficulty" control:_difficultyPopup]
	] spacing:RowSpacing];

	_gameplayKeyViews = @[
		_controlModePopup,
		_mouseSensitivitySlider,
		_invertMouseCheckbox,
		_joystickCheckbox,
		_autoCenterCameraCheckbox,
		_moveWhileCastingCheckbox,
		_autoQuaffCheckbox,
		_screenFlashesCheckbox,
		_difficultyPopup
	];
	[self controlModeChanged:nil];
	return [self scrollViewForForm:form];
}

- (NSWindow*)buildWindow
{
	const NSWindowStyleMask style = NSWindowStyleMaskTitled |
		NSWindowStyleMaskClosable |
		NSWindowStyleMaskMiniaturizable |
		NSWindowStyleMaskResizable;
	NSWindow* window = [[NSWindow alloc]
		initWithContentRect:NSMakeRect(0.0, 0.0, LauncherWindowWidth, LauncherWindowHeight)
		styleMask:style
		backing:NSBackingStoreBuffered
		defer:NO];
	window.title = @"Harry Potter 2 Launcher";
	window.contentMinSize = NSMakeSize(LauncherMinimumWidth, LauncherMinimumHeight);
	window.releasedWhenClosed = NO;
	window.tabbingMode = NSWindowTabbingModeDisallowed;
	window.delegate = self;
	window.autorecalculatesKeyViewLoop = NO;
	_window = window;

	NSView* contentView = window.contentView;

	NSTextField* heading = [self staticLabel:@"Harry Potter and the Chamber of Secrets"];
	heading.font = [NSFont systemFontOfSize:24.0 weight:NSFontWeightSemibold];
	[heading setAccessibilityLabel:@"Harry Potter and the Chamber of Secrets launcher"];
	heading.lineBreakMode = NSLineBreakByTruncatingTail;
	[heading setContentCompressionResistancePriority:NSLayoutPriorityDefaultLow
		forOrientation:NSLayoutConstraintOrientationHorizontal];
	_openUserFolderButton = [self
		utilityButtonWithTitle:@"Open User Folder"
		action:@selector(openUserFolder:)];
	_openUserFolderButton.enabled = !_request->userRoot.empty();
	[_openUserFolderButton setAccessibilityHelp:
		@"Opens the game's user-data folder in Finder without changing its contents."];

	_revealLogButton = [self utilityButtonWithTitle:@"Reveal Log" action:@selector(revealLog:)];
	BOOL logIsDirectory = NO;
	const BOOL logExists = !_request->logPath.empty() &&
		[[NSFileManager defaultManager]
			fileExistsAtPath:CocoaString(_request->logPath)
			isDirectory:&logIsDirectory];
	_revealLogButton.enabled = logExists && !logIsDirectory;
	[_revealLogButton setAccessibilityHelp:
		@"Reveals the existing game log in Finder without changing it."];

	NSStackView* utilityButtons = [self horizontalStackWithViews:@[
		_openUserFolderButton,
		_revealLogButton
	] spacing:ItemSpacing];
	[utilityButtons setHuggingPriority:NSLayoutPriorityRequired
		forOrientation:NSLayoutConstraintOrientationHorizontal];
	[heading setContentHuggingPriority:NSLayoutPriorityDefaultLow
		forOrientation:NSLayoutConstraintOrientationHorizontal];
	NSStackView* header = [self horizontalStackWithViews:@[
		heading,
		utilityButtons
	] spacing:SectionSpacing];
	header.distribution = NSStackViewDistributionFill;


	NSView* gameDataSection = [self buildGameDataSection];
	NSView* saveSection = [self buildSaveSection];

	NSBox* separator = [[NSBox alloc] initWithFrame:NSZeroRect];
	separator.translatesAutoresizingMaskIntoConstraints = NO;
	separator.boxType = NSBoxSeparator;

	_tabView = [[NSTabView alloc] initWithFrame:NSZeroRect];
	_tabView.translatesAutoresizingMaskIntoConstraints = NO;
	_tabView.delegate = self;
	[_tabView setAccessibilityLabel:@"Launcher settings"];

	NSTabViewItem* videoItem = [[NSTabViewItem alloc] initWithIdentifier:@"video"];
	videoItem.label = @"Video";
	videoItem.view = [self buildVideoTab];
	[_tabView addTabViewItem:videoItem];

	NSTabViewItem* audioItem = [[NSTabViewItem alloc] initWithIdentifier:@"audio"];
	audioItem.label = @"Audio";
	audioItem.view = [self buildAudioTab];
	[_tabView addTabViewItem:audioItem];

	NSTabViewItem* gameplayItem = [[NSTabViewItem alloc] initWithIdentifier:@"controls-gameplay"];
	gameplayItem.label = @"Controls & Gameplay";
	gameplayItem.view = [self buildControlsAndGameplayTab];
	[_tabView addTabViewItem:gameplayItem];
	[_tabView selectTabViewItem:videoItem];

	NSStackView* rootStack = [self verticalStackWithViews:@[
		header,
		gameDataSection,
		saveSection,
		separator,
		_tabView
	] spacing:SectionSpacing];
	rootStack.alignment = NSLayoutAttributeLeading;
	[rootStack setHuggingPriority:NSLayoutPriorityDefaultLow
		forOrientation:NSLayoutConstraintOrientationVertical];
	[contentView addSubview:rootStack];

	[NSLayoutConstraint activateConstraints:@[
		[rootStack.leadingAnchor constraintEqualToAnchor:contentView.leadingAnchor constant:OuterMargin],
		[rootStack.trailingAnchor constraintEqualToAnchor:contentView.trailingAnchor constant:-OuterMargin],
		[rootStack.topAnchor constraintEqualToAnchor:contentView.topAnchor constant:OuterMargin],
		[rootStack.bottomAnchor constraintEqualToAnchor:contentView.bottomAnchor constant:-OuterMargin],
		[header.widthAnchor constraintEqualToAnchor:rootStack.widthAnchor],
		[gameDataSection.widthAnchor constraintEqualToAnchor:rootStack.widthAnchor],
		[saveSection.widthAnchor constraintEqualToAnchor:rootStack.widthAnchor],
		[separator.widthAnchor constraintEqualToAnchor:rootStack.widthAnchor],
		[_tabView.widthAnchor constraintEqualToAnchor:rootStack.widthAnchor]
	]];
	[_tabView setContentHuggingPriority:NSLayoutPriorityDefaultLow
		forOrientation:NSLayoutConstraintOrientationVertical];
	[_tabView setContentCompressionResistancePriority:NSLayoutPriorityDefaultLow
		forOrientation:NSLayoutConstraintOrientationVertical];

	if (_continueButton.enabled)
	{
		_continueButton.keyEquivalent = @"\r";
		_continueButton.keyEquivalentModifierMask = 0;
		window.defaultButtonCell = _continueButton.cell;
	}
	else
	{
		_newGameButton.keyEquivalent = @"\r";
		_newGameButton.keyEquivalentModifierMask = 0;
		window.defaultButtonCell = _newGameButton.cell;
	}

	_mainKeyViews = @[
		_dataSourcePopup,
		_dataSourceRootField,
		_chooseDataFolderButton,
		_savePopup,
		_continueButton,
		_newGameButton,
		_quitButton,
		_openUserFolderButton,
		_revealLogButton,
		_tabView
	];
	[self updateSliderLabels];
	[self updateKeyViewLoop];
	window.initialFirstResponder = _continueButton.enabled ? _continueButton : _newGameButton;

	[window center];
	return window;
}

- (void)showAlertWithMessage:(NSString*)message informativeText:(NSString*)informativeText
{
	NSAlert* alert = [[NSAlert alloc] init];
	alert.alertStyle = NSAlertStyleWarning;
	alert.messageText = message;
	alert.informativeText = informativeText.length > 0
		? informativeText : @"An unknown launcher error occurred.";
	[alert addButtonWithTitle:@"OK"];
	[alert beginSheetModalForWindow:_window completionHandler:nil];
}

- (void)showInitialErrorIfNeeded
{
	if (_request->errorMessage.empty() || _request->hasExplicitDataRootOverride)
	{
		return;
	}
	[self
		showAlertWithMessage:@"Review Launcher Setup"
		informativeText:CocoaString(_request->errorMessage)];
}

- (void)updateSelectedSavePreview
{
	_thumbnailImageView.image = nil;
	_thumbnailImageView.hidden = YES;
	if (_request->saves.empty())
	{
		return;
	}

	const NSInteger selectedIndex = _savePopup.indexOfSelectedItem;
	if (selectedIndex < 0 ||
		static_cast<std::size_t>(selectedIndex) >= _request->saves.size())
	{
		return;
	}

	const HP2Launcher::SaveRecord& save = _request->saves[static_cast<std::size_t>(selectedIndex)];
	NSString* thumbnailPath = CocoaString(save.thumbnailPath);
	if (thumbnailPath.length == 0 ||
		[thumbnailPath.pathExtension caseInsensitiveCompare:@"bmp"] != NSOrderedSame)
	{
		return;
	}

	NSImage* thumbnail = [[NSImage alloc] initWithContentsOfFile:thumbnailPath];
	if (thumbnail == nil || !thumbnail.valid)
	{
		return;
	}

	_thumbnailImageView.image = thumbnail;
	_thumbnailImageView.hidden = NO;
	NSString* saveName = CocoaString(save.displayName);
	[_thumbnailImageView setAccessibilityLabel:saveName.length > 0
		? [NSString stringWithFormat:@"Thumbnail for %@", saveName]
		: @"Selected saved game thumbnail"];
}

- (HP2Launcher::DataSource)selectedDataSource
{
	const HP2Launcher::DataSource source =
		_dataSourcePopup != nil
			? static_cast<HP2Launcher::DataSource>(_dataSourcePopup.selectedItem.tag)
			: HP2Launcher::DataSource::Retail;
	return IsKnownDataSource(source) ? source : HP2Launcher::DataSource::Retail;
}

- (std::string)rootForDataSource:(HP2Launcher::DataSource)source
{
	return source == HP2Launcher::DataSource::Prototype
		? _dataSources.prototypeRoot
		: _dataSources.retailRoot;
}

- (const HP2Launcher::DataSourceOption*)dataSourceOptionForSource:(HP2Launcher::DataSource)source
{
	for (const HP2Launcher::DataSourceOption& option : _request->dataSourceOptions)
	{
		if (option.source == source)
		{
			return &option;
		}
	}
	return nullptr;
}

- (void)updateDataSourceControls
{
	const BOOL hasExplicitOverride = _request->hasExplicitDataRootOverride;
	const HP2Launcher::DataSource source = [self selectedDataSource];
	const std::string root = hasExplicitOverride
		? _request->explicitDataRoot
		: [self rootForDataSource:source];
	NSString* rootText = CocoaString(root);
	_dataSourceRootField.stringValue = rootText;
	[_dataSourceRootField setAccessibilityValue:rootText.length > 0
		? rootText
		: @"No game data folder selected"];

	if (hasExplicitOverride)
	{
		static NSString* const overrideHelp =
			@"Command-line data overrides launcher folders.";
		_dataSourcePopup.enabled = NO;
		_chooseDataFolderButton.enabled = NO;
		[_dataSourcePopup setAccessibilityHelp:overrideHelp];
		[_dataSourceRootField setAccessibilityHelp:overrideHelp];
		[_chooseDataFolderButton setAccessibilityHelp:overrideHelp];
		NSString* message = CocoaString(_request->errorMessage);
		_dataSourceStatusLabel.stringValue = message.length > 0 ? message : overrideHelp;
		[_dataSourceStatusLabel setAccessibilityValue:_dataSourceStatusLabel.stringValue];
		return;
	}

	_dataSourcePopup.enabled = YES;
	_chooseDataFolderButton.enabled = YES;
	[_dataSourcePopup setAccessibilityHelp:
		@"Choose the data source whose folder and separate profile you want to use."];
	[_dataSourceRootField setAccessibilityHelp:
		@"The selected source's game data folder. It must contain System/Default.ini."];
	[_chooseDataFolderButton setAccessibilityHelp:
		@"Choose a game data folder that contains System/Default.ini."];

	const HP2Launcher::DataSourceOption* option = [self dataSourceOptionForSource:source];
	NSString* status = @"";
	if (root.empty())
	{
		status = @"Choose a folder that contains System/Default.ini.";
	}
	else if (option != nullptr && !option->available && option->root == root)
	{
		status = CocoaString(option->error);
		if (status.length == 0)
		{
			status = @"The selected folder must contain System/Default.ini.";
		}
	}
	else
	{
		status = @"Retail and Prototype / Beta use separate saves and settings.";
	}
	_dataSourceStatusLabel.stringValue = status;
	[_dataSourceStatusLabel setAccessibilityValue:status];
}

- (HP2Launcher::DataSourceConfiguration)dataSourcesFromControls
{
	HP2Launcher::DataSourceConfiguration dataSources = _dataSources;
	dataSources.selected = [self selectedDataSource];
	return dataSources;
}
- (IBAction)dataSourceChanged:(id)sender
{
	(void)sender;
	[self updateDataSourceControls];
	[self updateKeyViewLoop];
}

- (IBAction)chooseDataFolder:(id)sender
{
	(void)sender;
	if (_request->hasExplicitDataRootOverride)
	{
		return;
	}

	NSOpenPanel* panel = [NSOpenPanel openPanel];
	panel.canChooseDirectories = YES;
	panel.canChooseFiles = NO;
	panel.allowsMultipleSelection = NO;
	panel.canCreateDirectories = NO;
	panel.prompt = @"Choose Folder";

	const std::string currentRoot = [self rootForDataSource:[self selectedDataSource]];
	NSString* currentPath = CocoaString(currentRoot);
	BOOL isDirectory = NO;
	if (currentPath.length > 0 &&
		[[NSFileManager defaultManager] fileExistsAtPath:currentPath isDirectory:&isDirectory] &&
		isDirectory)
	{
		panel.directoryURL = [NSURL fileURLWithPath:currentPath isDirectory:YES];
	}

	[panel beginSheetModalForWindow:_window completionHandler:^(NSModalResponse response)
	{
		if (response != NSModalResponseOK)
		{
			return;
		}

		NSURL* selectedURL = panel.URL;
		NSString* selectedPath = [[selectedURL path] stringByStandardizingPath];
		if (selectedURL == nil ||
			![selectedURL isFileURL] ||
			![selectedPath isAbsolutePath])
		{
			[self
				showAlertWithMessage:@"Game Data Folder Could Not Be Used"
				informativeText:@"Choose an absolute folder for the selected game data source."];
			return;
		}

		std::string selectedRoot;
		if (!CopyCocoaString(selectedPath, selectedRoot) || selectedRoot.empty())
		{
			[self
				showAlertWithMessage:@"Game Data Folder Could Not Be Used"
				informativeText:@"The selected folder name could not be read as UTF-8."];
			return;
		}

		if ([self selectedDataSource] == HP2Launcher::DataSource::Prototype)
		{
			_dataSources.prototypeRoot = std::move(selectedRoot);
		}
		else
		{
			_dataSources.retailRoot = std::move(selectedRoot);
		}
		[self updateDataSourceControls];
	}];
}


- (void)updateSliderLabels
{
	_brightnessValueLabel.stringValue = [NSString stringWithFormat:@"%.0f%%", _brightnessSlider.doubleValue * 100.0];
	_soundVolumeValueLabel.stringValue = [NSString stringWithFormat:@"%.0f%%", _soundVolumeSlider.doubleValue * 100.0];
	_musicVolumeValueLabel.stringValue = [NSString stringWithFormat:@"%.0f%%", _musicVolumeSlider.doubleValue * 100.0];
	_mouseSensitivityValueLabel.stringValue = [NSString stringWithFormat:@"%.1f", _mouseSensitivitySlider.doubleValue];
	[_brightnessSlider setAccessibilityValue:_brightnessValueLabel.stringValue];
	[_soundVolumeSlider setAccessibilityValue:_soundVolumeValueLabel.stringValue];
	[_musicVolumeSlider setAccessibilityValue:_musicVolumeValueLabel.stringValue];
	[_mouseSensitivitySlider setAccessibilityValue:_mouseSensitivityValueLabel.stringValue];
}

- (BOOL)isAvailableKeyView:(NSView*)view
{
	if (view == nil || view.hidden)
	{
		return NO;
	}
	if ([view isKindOfClass:NSControl.class] && ![(NSControl*)view isEnabled])
	{
		return NO;
	}
	return YES;
}

- (void)updateKeyViewLoop
{
	if (_window == nil || _mainKeyViews == nil)
	{
		return;
	}

	NSMutableArray<NSView*>* everyView = [NSMutableArray arrayWithArray:_mainKeyViews];
	[everyView addObjectsFromArray:_videoKeyViews ?: @[]];
	[everyView addObjectsFromArray:_audioKeyViews ?: @[]];
	[everyView addObjectsFromArray:_gameplayKeyViews ?: @[]];
	for (NSView* view in everyView)
	{
		view.nextKeyView = nil;
	}

	NSMutableArray<NSView*>* keyOrder = [NSMutableArray array];
	for (NSView* view in _mainKeyViews)
	{
		if ([self isAvailableKeyView:view])
		{
			[keyOrder addObject:view];
		}
	}

	NSArray<NSView*>* selectedTabViews = _videoKeyViews;
	const NSInteger selectedTabIndex = [_tabView indexOfTabViewItem:_tabView.selectedTabViewItem];
	if (selectedTabIndex == 1)
	{
		selectedTabViews = _audioKeyViews;
	}
	else if (selectedTabIndex == 2)
	{
		selectedTabViews = _gameplayKeyViews;
	}
	for (NSView* view in selectedTabViews)
	{
		if ([self isAvailableKeyView:view])
		{
			[keyOrder addObject:view];
		}
	}

	for (NSUInteger index = 0; index < keyOrder.count; ++index)
	{
		NSView* current = keyOrder[index];
		NSView* next = keyOrder[(index + 1) % keyOrder.count];
		current.nextKeyView = next;
	}
}

- (HP2Launcher::LauncherSettings)settingsFromControls
{
	HP2Launcher::LauncherSettings settings = _request->settings;
	settings.screenMode = static_cast<HP2Launcher::ScreenMode>(_screenModePopup.selectedItem.tag);
	settings.renderBackend = static_cast<HP2Launcher::RenderBackend>(_rendererPopup.selectedItem.tag);

	const NSInteger resolutionIndex = _resolutionPopup.indexOfSelectedItem;
	if (resolutionIndex >= 0 &&
		static_cast<std::size_t>(resolutionIndex) < _resolutions.size())
	{
		settings.resolution = _resolutions[static_cast<std::size_t>(resolutionIndex)];
	}

	settings.verticalSync = _verticalSyncCheckbox.state == NSControlStateValueOn;
	settings.renderScale = static_cast<double>(_renderScalePopup.selectedItem.tag) / 100.0;
	settings.uiScale = static_cast<double>(_uiScalePopup.selectedItem.tag) / 100.0;
	settings.frameRateLimit = static_cast<int>(_frameRatePopup.selectedItem.tag);
	settings.showFPS = _showFPSCheckbox.state == NSControlStateValueOn;
	settings.maintainVerticalFOV = _widescreenViewPopup.selectedItem.tag != 0;
	settings.nativeText = _textRenderingPopup.selectedItem.tag != 0;
	settings.antiAliasingSamples = static_cast<int>(_antiAliasingPopup.selectedItem.tag);
	settings.anisotropy = static_cast<int>(_anisotropyPopup.selectedItem.tag);
	settings.brightness = _brightnessSlider.doubleValue;
	settings.textureDetail = static_cast<HP2Launcher::TextureDetail>(_textureDetailPopup.selectedItem.tag);
	settings.objectDetail = static_cast<HP2Launcher::ObjectDetail>(_objectDetailPopup.selectedItem.tag);
	settings.soundEnabled = _soundEnabledCheckbox.state == NSControlStateValueOn;
	settings.soundVolume = _soundVolumeSlider.doubleValue;
	settings.musicVolume = _musicVolumeSlider.doubleValue;
	settings.mouseSensitivity = _mouseSensitivitySlider.doubleValue;
	settings.invertMouse = _invertMouseCheckbox.state == NSControlStateValueOn;
	settings.controlMode = static_cast<HP2Launcher::ControlMode>(_controlModePopup.selectedItem.tag);
	settings.joystickEnabled = _joystickCheckbox.state == NSControlStateValueOn;
	settings.autoCenterCamera = _autoCenterCameraCheckbox.state == NSControlStateValueOn;
	settings.moveWhileCasting = _moveWhileCastingCheckbox.state == NSControlStateValueOn;
	settings.autoQuaff = _autoQuaffCheckbox.state == NSControlStateValueOn;
	settings.screenFlashes = _screenFlashesCheckbox.state == NSControlStateValueOn;
	settings.difficulty = static_cast<HP2Launcher::Difficulty>(_difficultyPopup.selectedItem.tag);
	return settings;
}

- (BOOL)validateAndStoreSettings
{
	HP2Launcher::LauncherSettings settings = [self settingsFromControls];
	std::string validationError;
	if (!ValidateSettings(settings, validationError))
	{
		[self
			showAlertWithMessage:@"Review Launcher Settings"
			informativeText:CocoaString(validationError)];
		return NO;
	}
	_result.settings = std::move(settings);
	return YES;
}

- (void)finishWithAction:(HP2Launcher::LaunchAction)action validate:(BOOL)validate
{
	if (_completed)
	{
		return;
	}
	if (validate)
	{
		if (![self validateAndStoreSettings])
		{
			return;
		}
	}
	else
	{
		_result.settings = [self settingsFromControls];
	}

	_result.selection = HP2Launcher::LaunchSelection{};
	_result.selection.action = action;
	if (action == HP2Launcher::LaunchAction::Continue)
	{
		const NSInteger selectedIndex = _savePopup.indexOfSelectedItem;
		if (selectedIndex < 0 ||
			static_cast<std::size_t>(selectedIndex) >= _request->saves.size())
		{
			[self
				showAlertWithMessage:@"Choose a Saved Game"
				informativeText:@"Select the saved game you want to continue."];
			return;
		}
		_result.selection.hasSave = true;
		_result.selection.save = _request->saves[static_cast<std::size_t>(selectedIndex)];
	}

	if (action == HP2Launcher::LaunchAction::Continue ||
		action == HP2Launcher::LaunchAction::NewGame)
	{
		_result.dataSources = [self dataSourcesFromControls];
	}

	_completed = YES;
	[_application stopModal];
}

- (IBAction)continueGame:(id)sender
{
	(void)sender;
	if (_request->saves.empty())
	{
		return;
	}
	[self finishWithAction:HP2Launcher::LaunchAction::Continue validate:YES];
}

- (IBAction)newGame:(id)sender
{
	(void)sender;
	[self finishWithAction:HP2Launcher::LaunchAction::NewGame validate:YES];
}

- (IBAction)quit:(id)sender
{
	(void)sender;
	[self finishWithAction:HP2Launcher::LaunchAction::Quit validate:NO];
}
- (IBAction)openUserFolder:(id)sender
{
	(void)sender;
	NSString* path = CocoaString(_request->userRoot);
	BOOL isDirectory = NO;
	if (path.length == 0 ||
		![[NSFileManager defaultManager] fileExistsAtPath:path isDirectory:&isDirectory] ||
		!isDirectory)
	{
		[self
			showAlertWithMessage:@"User Folder Is Unavailable"
			informativeText:@"The game's user-data folder could not be found."];
		return;
	}
	if (![[NSWorkspace sharedWorkspace] openURL:[NSURL fileURLWithPath:path isDirectory:YES]])
	{
		[self
			showAlertWithMessage:@"User Folder Could Not Be Opened"
			informativeText:@"Finder could not open the game's user-data folder."];
	}
}

- (IBAction)revealLog:(id)sender
{
	(void)sender;
	NSString* path = CocoaString(_request->logPath);
	BOOL isDirectory = NO;
	if (path.length == 0 ||
		![[NSFileManager defaultManager] fileExistsAtPath:path isDirectory:&isDirectory] ||
		isDirectory)
	{
		_revealLogButton.enabled = NO;
		[self
			showAlertWithMessage:@"Game Log Is Unavailable"
			informativeText:@"No game log is available to reveal."];
		return;
	}
	if (![[NSWorkspace sharedWorkspace] selectFile:path inFileViewerRootedAtPath:@""])
	{
		[self
			showAlertWithMessage:@"Game Log Could Not Be Revealed"
			informativeText:@"Finder could not reveal the game log."];
	}
}


- (IBAction)saveSelectionChanged:(id)sender
{
	(void)sender;
	[self updateSelectedSavePreview];
}

- (void)rebuildResolutionPopupForMode:(HP2Launcher::ScreenMode)mode
{
	const NSInteger oldIndex = _resolutionPopup.indexOfSelectedItem;
	if (_displayedScreenMode != HP2Launcher::ScreenMode::BorderlessDesktop &&
		oldIndex >= 0 &&
		static_cast<std::size_t>(oldIndex) < _resolutions.size())
	{
		const HP2Launcher::DisplayResolution selected =
			_resolutions[static_cast<std::size_t>(oldIndex)];
		if (_displayedScreenMode == HP2Launcher::ScreenMode::Windowed)
		{
			_windowedSelection = selected;
		}
		else if (_displayedScreenMode == HP2Launcher::ScreenMode::Fullscreen)
		{
			_fullscreenSelection = selected;
		}
	}

	[_resolutionPopup removeAllItems];
	const HP2Launcher::DisplayResolution* preferred = nullptr;
	if (mode == HP2Launcher::ScreenMode::BorderlessDesktop)
	{
		_resolutions.assign(1, _currentDisplayResolution);
		preferred = &_currentDisplayResolution;
	}
	else if (mode == HP2Launcher::ScreenMode::Fullscreen)
	{
		_resolutions = _fullscreenResolutions;
		preferred = &_fullscreenSelection;
	}
	else
	{
		_resolutions = _windowedResolutions;
		preferred = &_windowedSelection;
	}

	NSInteger selectedIndex = -1;
	for (std::size_t index = 0; index < _resolutions.size(); ++index)
	{
		const HP2Launcher::DisplayResolution& resolution = _resolutions[index];
		[_resolutionPopup addItemWithTitle:CocoaString(resolution.label)];
		if (preferred != nullptr &&
			resolution.width == preferred->width &&
			resolution.height == preferred->height)
		{
			selectedIndex = static_cast<NSInteger>(index);
		}
	}
	if (selectedIndex < 0 && mode == HP2Launcher::ScreenMode::Fullscreen)
	{
		for (std::size_t index = 0; index < _resolutions.size(); ++index)
		{
			if (_resolutions[index].width == _currentDisplayResolution.width &&
				_resolutions[index].height == _currentDisplayResolution.height)
			{
				selectedIndex = static_cast<NSInteger>(index);
				break;
			}
		}
	}
	if (selectedIndex < 0 && mode == HP2Launcher::ScreenMode::Windowed)
	{
		for (std::size_t index = 0; index < _resolutions.size(); ++index)
		{
			if (_resolutions[index].width == 800 && _resolutions[index].height == 600)
			{
				selectedIndex = static_cast<NSInteger>(index);
				break;
			}
		}
	}
	if (selectedIndex < 0 && !_resolutions.empty())
	{
		selectedIndex = 0;
	}
	if (selectedIndex >= 0)
	{
		[_resolutionPopup selectItemAtIndex:selectedIndex];
		const HP2Launcher::DisplayResolution selected =
			_resolutions[static_cast<std::size_t>(selectedIndex)];
		if (mode == HP2Launcher::ScreenMode::Windowed)
		{
			_windowedSelection = selected;
		}
		else if (mode == HP2Launcher::ScreenMode::Fullscreen)
		{
			_fullscreenSelection = selected;
		}
	}
	_displayedScreenMode = mode;
}

- (IBAction)displayModeChanged:(id)sender
{
	(void)sender;
	const HP2Launcher::ScreenMode mode =
		static_cast<HP2Launcher::ScreenMode>(_screenModePopup.selectedItem.tag);
	[self rebuildResolutionPopupForMode:mode];
	const BOOL usesChosenResolution = mode != HP2Launcher::ScreenMode::BorderlessDesktop;
	_resolutionPopup.enabled = usesChosenResolution;
	if (mode == HP2Launcher::ScreenMode::Windowed)
	{
		[_screenModePopup setAccessibilityHelp:
			@"Runs in a resizable desktop window at the selected logical resolution."];
		[_resolutionPopup setAccessibilityHelp:
			@"Only window sizes that fit the main screen's visible desktop are listed."];
	}
	else if (mode == HP2Launcher::ScreenMode::Fullscreen)
	{
		[_screenModePopup setAccessibilityHelp:
			@"Uses an exclusive fullscreen display mode at the selected logical resolution."];
		[_resolutionPopup setAccessibilityHelp:
			@"Only logical modes supported by the main display are listed."];
	}
	else
	{
		[_screenModePopup setAccessibilityHelp:
			@"Fills the desktop without changing the main display's current mode."];
		[_resolutionPopup setAccessibilityHelp:
			@"Borderless Desktop always uses the main display's current logical resolution."];
	}
	[self updateKeyViewLoop];
}

- (IBAction)controlModeChanged:(id)sender
{
	(void)sender;
	const BOOL modern =
		_controlModePopup.selectedItem.tag == static_cast<NSInteger>(HP2Launcher::ControlMode::Modern);
	_autoCenterCameraCheckbox.enabled = !modern;
	[self updateKeyViewLoop];
}

- (IBAction)soundEnabledChanged:(id)sender
{
	(void)sender;
	const BOOL enabled = _soundEnabledCheckbox.state == NSControlStateValueOn;
	_soundVolumeSlider.enabled = enabled;
	_musicVolumeSlider.enabled = enabled;
	_soundVolumeValueLabel.textColor = enabled
		? NSColor.secondaryLabelColor : NSColor.disabledControlTextColor;
	_musicVolumeValueLabel.textColor = enabled
		? NSColor.secondaryLabelColor : NSColor.disabledControlTextColor;
	[self updateKeyViewLoop];
}

- (IBAction)sliderChanged:(id)sender
{
	(void)sender;
	[self updateSliderLabels];
}

- (BOOL)windowShouldClose:(NSWindow*)sender
{
	(void)sender;
	[self finishWithAction:HP2Launcher::LaunchAction::Quit validate:NO];
	return NO;
}

- (void)tabView:(NSTabView*)tabView didSelectTabViewItem:(NSTabViewItem*)tabViewItem
{
	(void)tabView;
	(void)tabViewItem;
	[self updateKeyViewLoop];
}

- (void)copyResultTo:(HP2Launcher::LauncherResult*)result
{
	if (result != nullptr)
	{
		*result = _result;
	}
}

- (void)finalizeAsQuitIfNeeded
{
	if (!_completed)
	{
		_result.settings = [self settingsFromControls];
		_result.selection = HP2Launcher::LaunchSelection{};
		_result.selection.action = HP2Launcher::LaunchAction::Quit;
		_completed = YES;
	}
}

- (void)detach
{
	if (_window.delegate == self)
	{
		_window.delegate = nil;
	}
	_tabView.delegate = nil;
	_window = nil;
	_application = nil;
	_request = nullptr;
}

@end

namespace HP2Launcher
{
LaunchAction RunHP2MacLauncher(
	const LauncherRequest& request,
	LauncherResult& result,
	std::string& error)
{
	result = LauncherResult{};
	result.settings = request.settings;
	result.selection.action = LaunchAction::Error;
	error.clear();

	LaunchAction action = LaunchAction::Error;
	@autoreleasepool
	{
		const BOOL isMainThread = NSThread.isMainThread;
		NSCAssert(isMainThread, @"RunHP2MacLauncher must run on the main thread.");
		if (!isMainThread)
		{
			error = "The macOS launcher must run on the process main thread.";
			return LaunchAction::Error;
		}

		NSApplication* application = nil;
		HP2MacLauncherController* controller = nil;
		NSWindow* window = nil;
		NSMenu* previousMainMenu = nil;
		NSMenu* launcherMainMenu = nil;
		NSMenuItem* quitMenuItem = nil;

		@try
		{
			application = [NSApplication sharedApplication];
			[application setActivationPolicy:NSApplicationActivationPolicyRegular];
			[application finishLaunching];

			controller = [[HP2MacLauncherController alloc]
				initWithRequest:&request
				application:application];
			window = [controller buildWindow];
			if (window == nil)
			{
				error = "The macOS launcher window could not be created.";
			}
			else
			{
				previousMainMenu = application.mainMenu;
				launcherMainMenu = [[NSMenu alloc] initWithTitle:@""];
				NSMenuItem* applicationMenuItem = [[NSMenuItem alloc]
					initWithTitle:@"Harry Potter 2"
					action:nil
					keyEquivalent:@""];
				NSMenu* applicationMenu = [[NSMenu alloc] initWithTitle:@"Harry Potter 2"];
				quitMenuItem = [[NSMenuItem alloc]
					initWithTitle:@"Quit Harry Potter 2"
					action:@selector(quit:)
					keyEquivalent:@"q"];
				quitMenuItem.keyEquivalentModifierMask = NSEventModifierFlagCommand;
				quitMenuItem.target = controller;
				[applicationMenu addItem:quitMenuItem];
				applicationMenuItem.submenu = applicationMenu;
				[launcherMainMenu addItem:applicationMenuItem];
				application.mainMenu = launcherMainMenu;

				[window makeKeyAndOrderFront:nil];
				[application activateIgnoringOtherApps:YES];
				[controller showInitialErrorIfNeeded];
				[application runModalForWindow:window];
				[controller finalizeAsQuitIfNeeded];
				[controller copyResultTo:&result];
				action = result.selection.action;
				error.clear();
			}
		}
		@catch (NSException* exception)
		{
			if (application != nil && application.modalWindow == window)
			{
				[application abortModal];
			}
			NSString* reason = exception.reason;
			const char* utf8Reason = reason.UTF8String;
			error = utf8Reason != nullptr
				? std::string(utf8Reason)
				: std::string("The macOS launcher could not be displayed.");
			result = LauncherResult{};
			result.settings = request.settings;
			result.selection.action = LaunchAction::Error;
			action = LaunchAction::Error;
		}
		@finally
		{
			quitMenuItem.target = nil;
			if (application != nil && application.mainMenu == launcherMainMenu)
			{
				application.mainMenu = previousMainMenu;
			}
			if (window != nil)
			{
				window.delegate = nil;
				[window orderOut:nil];
				[window close];
			}
			[controller detach];
			quitMenuItem = nil;
			launcherMainMenu = nil;
			previousMainMenu = nil;
			window = nil;
			controller = nil;
			application = nil;
		}
	}
	return action;
}
}
