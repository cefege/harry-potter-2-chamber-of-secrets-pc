class FESoundVideoPage expands baseFEPage;

#EXEC TEXTURE IMPORT NAME=FEOverOptionTexture	 FILE=TEXTURES\Menu\Options\overoption.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEOverOption3Texture	 FILE=TEXTURES\Menu\Options\overoption3.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
//#EXEC TEXTURE IMPORT NAME=FEOverOption4Texture	 FILE=TEXTURES\Menu\Options\overoption4.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEOverOption5Texture	 FILE=TEXTURES\Menu\Options\overoption5.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

// RenderDriver
var string					GameRenderDriver;


var bool					bInitialized;
var string					OldSettings;

// Options label
var UWindowButton			OptionsLabel;
var localized string		optionsText;
							
							
var UWindowLabelControl		VideoLabel;
var localized string		videoText;
							
// Resolution		
var HPMenuOptionCombo		ResolutionCombo;
var localized string		ResolutionText;
							
// Color Depth				
var HPMenuOptionCombo		ColorDepthCombo;
var localized string		ColorDepthText;
var localized string		BitsText;

// Brightness
var UWindowLabelControl		BrightnessLabel;
var UWindowLabelControl		BrightnessHiText;
var UWindowLabelControl		BrightnessLoText;
var UWindowHSliderControl	BrightnessSlider;
var localized string		BrightnessText;


var localized string		DetailLevel[5];
							
// Texture Detail			
var HPMenuOptionCombo		TextureDetailCombo;
var localized string		TextureDetailText;
var int						OldTextureDetail;
							
// Object Detail
var UWindowLabelControl		ObjectDetailLabel;
var UWindowLabelControl		ObjectDetailHiText;
var UWindowLabelControl		ObjectDetailLoText;		
var UWindowHSliderControl	ObjectDetailSlider;
var localized string		ObjectDetailText;

							
var UWindowLabelControl		AudioLabel;
var localized string		audioText;

// Music Volume
var UWindowHSliderControl	MusicVolumeSlider;
var UWindowLabelControl		MusicVolumeLabel;
var localized string		MusicVolumeText;
var UWindowLabelControl		MusicVolumeHiText;
var UWindowLabelControl		MusicVolumeLoText;

// Sound Volume
var UWindowHSliderControl	SoundVolumeSlider;
var UWindowLabelControl		SoundVolumeLabel;
var localized string		SoundVolumeText;
var UWindowLabelControl		SoundVolumeHiText;
var UWindowLabelControl		SoundVolumeLoText;

var localized string		VolumeHiText;
var localized string		VolumeLoText;



var HPMessageBox			ConfirmSettings;

var localized string		ConfirmSettingsTitle;
var localized string		ConfirmSettingsText;
var localized string		ConfirmSettingsCancelTitle;
var localized string		ConfirmSettingsCancelText;


var UWindowButton			SelectedButton;
var bool					bPolling;
var int						Selection;


// Difficulty
//var UWindowComboControl DifficultyCombo;
var localized string		DifficultyText;
var localized string		DifficultyLevel[3];

var Color					ButtonTextColor, LabelTextColor, GoupLabelTextColor;
var int						vertSpacing [8];

var sound					buttonClickSound;


function LocalizeStrings()
{
	local int i;
	local string tmpStr;

	// options_07	High
	VolumeHiText=GetLocalFEString("Options_0002");	//"High"
	// options_10	Low
	VolumeLoText=GetLocalFEString("Options_0005");	//"Low"

	// options_02	Resolution
	ResolutionText=GetLocalFEString("Options_0033");	//"Resolution"

	// options_03	Colour Depth
	ColorDepthText=GetLocalFEString("Options_0026");	//"Color Depth"

	//options_04	Brightness
	BrightnessText=GetLocalFEString("Options_0006");	//"Brightness"

	//options_05	Texture Detail
	TextureDetailText=GetLocalFEString("Options_0000");	//"Texture Detail"

	// options_12	Object Detail
	ObjectDetailText=GetLocalFEString("Options_0007");	//"Object Detail"

	// options_13	Audio
	audioText=GetLocalFEString("Options_0009");	//"Audio"

	// options_14	Music Volume
	MusicVolumeText=GetLocalFEString("Options_0010");	//"Music Volume"

	// options_15	Sound Volume
	SoundVolumeText=GetLocalFEString("Options_0011");	//"Sound Volume"

	// options_06	Very High
	DetailLevel[0]=GetLocalFEString("Options_0001");	//"Very High"

	// options_07	High
	DetailLevel[1]=GetLocalFEString("Options_0002");	//"High"
	// options_08	Medium
	DetailLevel[2]=GetLocalFEString("Options_0003");	//"Medium"
	// options_10	Low
	DetailLevel[3]=GetLocalFEString("Options_0005");	//"Low"

	// options_11	Very Low
	DetailLevel[4]=GetLocalFEString("Options_0016");	//"Very Low"

	// options_01	Options
	optionsText=GetLocalFEString("Options_0024");	//"OPTIONS"


/***************Not Used**************************/

	DifficultyText="Difficulty";
	DifficultyLevel[0]="Easy";
	DifficultyLevel[1]="Medium";
	DifficultyLevel[2]="Hard";
/***************Not Used**************************/

	videoText					= GetLocalFEString("Options_0044");	// "Video"
	BitsText					= GetLocalFEString("Options_0051");	// "bit"
	
	ConfirmSettingsTitle		= GetLocalFEString("Options_0047");	// "Confirm Video Settings Change";
	ConfirmSettingsText			= GetLocalFEString("Options_0048");	// "Are you sure you wish to keep these new video settings?";
	ConfirmSettingsCancelTitle	= GetLocalFEString("Options_0049");	// "Video Settings Change";
	ConfirmSettingsCancelText	= GetLocalFEString("Options_0050");	// "Your previous video settings have been restored.";
}


function Created()
{
	local int ctlX, ctlY, ctlW, ctlH, labelWidth, labelX, offsetX, offsetY, I;
	local int MusicVolume, SoundVolume;
	local string sens;

	LocalizeStrings();

/*	OptionsLabel = UWindowButton(CreateControl(class'UWindowButton', 264-140, 40, 280, 25));
	OptionsLabel.SetText(optionsText);
	OptionsLabel.Align=TA_Center;
	OptionsLabel.SetFont(F_HPMenuLarge);
	OptionsLabel.TextColor = LabelTextColor;
	OptionsLabel.ShowWindow();
*/
	offsetX = 0;
	offsetY = 0;
	
	ctlX   = 180-offsetX;
	labelX = ctlX-80;

	ctlY = 90-offsetY;
	ctlH = 17;
	ctlW = 134;
	labelWidth = 50+ctlX-labelX;


	VideoLabel = UWindowLabelControl(CreateControl(class'UWindowLabelControl', ctlX, ctlY, ctlW, 1));
	VideoLabel.SetText(VideoText);
	VideoLabel.SetFont(F_Bold);
	VideoLabel.TextColor = GoupLabelTextColor;
	ctlY += 40;

	I = 0;

	// Resolution
	ResolutionCombo = HPMenuOptionCombo(CreateControl(class'HPMenuOptionCombo', labelX-18, ctlY, 30+ctlX+ctlW-LabelX, 1));
	ResolutionCombo.SetText(ResolutionText);
	ResolutionCombo.SetFont(F_Bold);
	ResolutionCombo.SetEditable(False);
	ResolutionCombo.EditBoxWidth = ctlW;
	ResolutionCombo.TextColor = LabelTextColor;
	ResolutionCombo.SetEditTextColor( ButtonTextColor );
	ctlY += vertSpacing[I++];

	ColorDepthCombo = HPMenuOptionCombo(CreateControl(class'HPMenuOptionCombo', labelX-18, ctlY, 30+ctlX+ctlW-LabelX, 1));
	ColorDepthCombo.SetText(ColorDepthText);
	ColorDepthCombo.SetFont(F_Bold);
	ColorDepthCombo.SetEditable(False);
	ColorDepthCombo.EditBoxWidth = ctlW;
	ColorDepthCombo.TextColor = LabelTextColor;
	ColorDepthCombo.SetEditTextColor( ButtonTextColor );
	ctlY += vertSpacing[I++];

	// Texture Detail
	TextureDetailCombo = HPMenuOptionCombo(CreateControl(class'HPMenuOptionCombo', labelX-18, ctlY, 30+ctlX+ctlW-LabelX, 1));
	TextureDetailCombo.SetText(TextureDetailText);
	TextureDetailCombo.SetFont(F_Bold);
	TextureDetailCombo.SetEditable(False);
	TextureDetailCombo.EditBoxWidth = ctlW;
	TextureDetailCombo.TextColor = LabelTextColor;
	TextureDetailCombo.SetEditTextColor( ButtonTextColor );
	
	// The display names are localized.  These strings match the enums in UnCamMgr.cpp.
	TextureDetailCombo.AddItem(DetailLevel[1], "High");
	TextureDetailCombo.AddItem(DetailLevel[2], "Medium");
	TextureDetailCombo.AddItem(DetailLevel[3], "Low");

	ctlY += 5 + vertSpacing[I++];
	

	// Object Detail
	ObjectDetailLabel = UWindowLabelControl(CreateControl(class'UWindowLabelControl', labelX, ctlY+10, labelWidth, ctlH));
	ObjectDetailLabel.SetText( ObjectDetailText );
	ObjectDetailLabel.SetFont(F_Bold);
	ObjectDetailLabel.TextColor = LabelTextColor;	

	ObjectDetailSlider = HPMenuOptionHSlider(CreateControl(class'HPMenuOptionHSlider', labelX, ctlY, 10+ctlX+ctlW-LabelX+4, 1));
	ObjectDetailSlider.bNoSlidingNotify = True;
	ObjectDetailSlider.SetRange(0, 4, 1);
//	ObjectDetailSlider.SetText(ObjectDetailText);
//	ObjectDetailSlider.SetFont( F_Bold );
//	ObjectDetailSlider.TextColor = LabelTextColor;
	ctlY += 32;
	
	ObjectDetailHiText = UWindowLabelControl(CreateControl(class'UWindowLabelControl', ctlX+120, ctlY, ctlW, 1));
	ObjectDetailHiText.SetText(VolumeHiText);
	ObjectDetailHiText.SetFont(F_Normal);
	ObjectDetailHiText.TextColor = LabelTextColor;
	
	ObjectDetailLoText = UWindowLabelControl(CreateControl(class'UWindowLabelControl', ctlX, ctlY, ctlW, 1));
	ObjectDetailLoText.SetText(VolumeLoText);
	ObjectDetailLoText.SetFont(F_Normal);
	ObjectDetailLoText.TextColor = LabelTextColor;
	ctlY += 32;

	// Brightness
	BrightnessLabel = UWindowLabelControl(CreateControl(class'UWindowLabelControl', labelX, ctlY+10, labelWidth, ctlH));
	BrightnessLabel.SetText( BrightnessText );
	BrightnessLabel.SetFont(F_Bold);
	BrightnessLabel.TextColor = LabelTextColor;	

	BrightnessSlider = HPMenuOptionHSlider(CreateControl(class'HPMenuOptionHSlider', labelX, ctlY, 10+ctlX+ctlW-LabelX+4, 1));
	BrightnessSlider.bNoSlidingNotify = True;
	BrightnessSlider.SetRange(2, 10, 1);
//	BrightnessSlider.SetText(BrightnessText);
//	BrightnessSlider.SetFont( F_Bold );
//	BrightnessSlider.TextColor = LabelTextColor;
	ctlY += 32;
	
	BrightnessHiText = UWindowLabelControl(CreateControl(class'UWindowLabelControl', ctlX+120, ctlY, ctlW, 1));
	BrightnessHiText.SetText(VolumeHiText);
	BrightnessHiText.SetFont(F_Normal);
	BrightnessHiText.TextColor = LabelTextColor;
	
	BrightnessLoText = UWindowLabelControl(CreateControl(class'UWindowLabelControl', ctlX, ctlY, ctlW, 1));
	BrightnessLoText.SetText(VolumeLoText);
	BrightnessLoText.SetFont(F_Normal);
	BrightnessLoText.TextColor = LabelTextColor;
	ctlY += vertSpacing[I++];


	// Set to Right-hand page
	//-----------------------
	ctlY   = 90 - offsetY;
	ctlX   = 380 - offsetX;
	labelX = 470 - offsetX;

	// --- Audio Settings
	
	// Audio label
  	AudioLabel = UWindowLabelControl(CreateControl(class'UWindowLabelControl', ctlX, ctlY, ctlW, 1));
	AudioLabel.SetText(AudioText);
	AudioLabel.SetFont(F_Bold);
	AudioLabel.TextColor = GoupLabelTextColor;
	ctlY += 30;
	
	// Music Volume
	MusicVolumeLabel = UWindowLabelControl(CreateControl(class'UWindowLabelControl', ctlX, ctlY+10, labelWidth, ctlH));
	MusicVolumeLabel.SetText( MusicVolumeText $" - " $int(MusicVolumeSlider.Value) );
	MusicVolumeLabel.SetFont(F_Bold);
	MusicVolumeLabel.TextColor = LabelTextColor;	
	ctlY += 30;

	MusicVolume		  = int(float(GetPlayerOwner().ConsoleCommand("get ini:Engine.Engine.AudioDevice MusicVolume"))*100);
	MusicVolumeSlider = HPMenuOptionHSlider(CreateControl(class'HPMenuOptionHSlider', ctlX, ctlY, ctlW, 1));
	MusicVolumeSlider.SetRange( 0, 100, 1 );
	MusicVolumeSlider.SetValue( MusicVolume );
	MusicVolumeSlider.SetText("");
	ctlY += 32;
	
	MusicVolumeHiText = UWindowLabelControl(CreateControl(class'UWindowLabelControl', ctlX+110, ctlY, ctlW, 1));
	MusicVolumeHiText.SetText(VolumeHiText);
	MusicVolumeHiText.SetFont(F_Normal);
	MusicVolumeHiText.TextColor = LabelTextColor;
	
	MusicVolumeLoText = UWindowLabelControl(CreateControl(class'UWindowLabelControl', ctlX, ctlY, ctlW, 1));
	MusicVolumeLoText.SetText(VolumeLoText);
	MusicVolumeLoText.SetFont(F_Normal);
	MusicVolumeLoText.TextColor = LabelTextColor;
	ctlY += 30;
	
	log("Options::SoundVideoPage: MusicVolume " $MusicVolume );
	
	// Sound Volume
	SoundVolumeLabel = UWindowLabelControl(CreateControl(class'UWindowLabelControl', ctlX, ctlY+10, labelWidth, ctlH));
	SoundVolumeLabel.SetText( SoundVolumeText $" - " $int(SoundVolumeSlider.Value) );
	SoundVolumeLabel.SetFont(F_Bold);
	SoundVolumeLabel.TextColor = LabelTextColor;
	ctlY += 30;

	SoundVolume		  = int(float(GetPlayerOwner().ConsoleCommand("get ini:Engine.Engine.AudioDevice SoundVolume"))*100);
	SoundVolumeSlider = HPMenuOptionHSlider(CreateControl(class'HPMenuOptionHSlider', ctlX , ctlY, ctlW, 1));	
	SoundVolumeSlider.SetRange( 0, 100, 1 );
	SoundVolumeSlider.SetValue( SoundVolume );
	SoundVolumeSlider.SetText("");
	ctlY += 32;

	SoundVolumeHiText = UWindowLabelControl(CreateControl(class'UWindowLabelControl', ctlX+110, ctlY, ctlW, 1));
	SoundVolumeHiText.SetText(VolumeHiText);
	SoundVolumeHiText.SetFont(F_Normal);
	SoundVolumeHiText.TextColor = LabelTextColor;
	
	SoundVolumeLoText = UWindowLabelControl(CreateControl(class'UWindowLabelControl', ctlX, ctlY, ctlW, 1));
	SoundVolumeLoText.SetText(VolumeLoText);
	SoundVolumeLoText.SetFont(F_Normal);
	SoundVolumeLoText.TextColor = LabelTextColor;
	ctlY += 38;
	
	log("Options::SoundVideoPage: MusicVolume " $SoundVolume );
	

	LoadAvailableSettings();
	
	// Create our BackPage button
	CreateBackPageButton();
}

function PlayClick()
{
    if( buttonClickSound != None )
    {
        GetPlayerOwner().PlaySound( buttonClickSound, SLOT_Interact );
    }
}
 

function bool IsSupportedResolution( string TempStr )
{
	if( GetPlayerOwner().IsSoftwareRendering() )
	{
		if( TempStr~="512x384" )//|| TempStr~="640x480" || TempStr~="800x600" || TempStr~="1024x768" )
			return true;
	}
	else
	{
		if( TempStr~="640x480" || TempStr~="800x600" || TempStr~="1024x768" || TempStr~="1280x1024")
			return true;
	}
	
	// This resolution is not supported
	return false;
}

function LoadAvailableSettings()
{
	local float Brightness;
	local string ParseString;
	local string CurrentDepth;
	local int P, I;
	local string TempStr;
	local string strStartupFullscreen;
	local string strCurrentRes;
	
	// Get the GameRenderDriver
	GameRenderDriver = GetPlayerOwner().ConsoleCommand("get ini:Engine.Engine GameRenderDevice");		
	
	bInitialized = false;

	// Load available video drivers and current video driver here.
	
	ResolutionCombo.Clear();

	GetPlayerOwner().ClientMessage("GameRenderDriver:" $GameRenderDriver  );

	if( GetPlayerOwner().IsSoftwareRendering() )
	{
		// we only allow one resolution in software mode
		ResolutionCombo.AddItem("512x384");
		ResolutionCombo.SetValue("512x384");
	}
	else
	{
		// Hardware mode
		ParseString = GetPlayerOwner().ConsoleCommand("GetRes");
		
		P = InStr(ParseString, " ");
		while(P != -1) 
		{
			// limit to supported resolutions
			TempStr = Left(ParseString, P);

			if (IsSupportedResolution(TempStr))
				ResolutionCombo.AddItem(Left(ParseString, P));

			ParseString = Mid(ParseString, P+1);
			P = InStr(ParseString, " ");
		}

		// limit to supported resolutions
		if (IsSupportedResolution(ParseString))
			ResolutionCombo.AddItem(ParseString);
		
		strStartupFullscreen = GetPlayerOwner().ConsoleCommand("get ini:Engine.Engine.ViewportManager StartupFullscreen");
		
		
		if( strStartupFullscreen == "True" )
		{
			// startup fullscreen
			strCurrentRes = GetPlayerOwner().ConsoleCommand("get ini:Engine.Engine.ViewportManager FullscreenViewportX");	
			strCurrentRes = strCurrentRes$"x";
			strCurrentRes = strCurrentRes$GetPlayerOwner().ConsoleCommand("get ini:Engine.Engine.ViewportManager FullscreenViewportY");
		}
		else
		{
			// startup windowed
			strCurrentRes = GetPlayerOwner().ConsoleCommand("get ini:Engine.Engine.ViewportManager WindowedViewportX");	
			strCurrentRes = strCurrentRes$"x";
			strCurrentRes = strCurrentRes$GetPlayerOwner().ConsoleCommand("get ini:Engine.Engine.ViewportManager WindowedViewportY");
		}
		
		GetPlayerOwner().ClientMessage("strStartupFullscreen:" $strStartupFullscreen $"strCurrentRes:" $strCurrentRes  );
		
		// set our current res
		ResolutionCombo.SetValue( strCurrentRes );
//		ResolutionCombo.SetValue( GetPlayerOwner().ConsoleCommand("GetCurrentRes") );

	}

	ColorDepthCombo.Clear();

	if( GetPlayerOwner().IsSoftwareRendering() )
	{
		// software renderer
		// only allow the first selection ( 16-bit)
		ParseString = GetPlayerOwner().ConsoleCommand("GetColorDepths");
		P = InStr(ParseString, " ");
		if(P != -1) 
		{
			ColorDepthCombo.AddItem(Left(ParseString, P)@BitsText, Left(ParseString, P));
			ColorDepthCombo.SetValue(Left(ParseString, P)@BitsText);
		}
	}
	else
	{
		// hardware renderer
		ParseString = GetPlayerOwner().ConsoleCommand("GetColorDepths");
		P = InStr(ParseString, " ");
		while (P != -1) 
		{
			ColorDepthCombo.AddItem(Left(ParseString, P)@BitsText, Left(ParseString, P));
			ParseString = Mid(ParseString, P+1);
			P = InStr(ParseString, " ");
		}
		ColorDepthCombo.AddItem(ParseString@BitsText, ParseString);
		CurrentDepth = GetPlayerOwner().ConsoleCommand("GetCurrentColorDepth");
		ColorDepthCombo.SetValue(CurrentDepth@BitsText, CurrentDepth);
	}

	Brightness = int(float(GetPlayerOwner().ConsoleCommand("get ini:Engine.Engine.ViewportManager Brightness")) * 10);
	BrightnessSlider.SetValue(Brightness);

	OldTextureDetail = Max(0, TextureDetailCombo.FindItemIndex2(GetPlayerOwner().ConsoleCommand("get ini:Engine.Engine.ViewportManager TextureDetail")));
	TextureDetailCombo.SetSelectedIndex(OldTextureDetail);
	
	// update the detail slider
	switch( GetPlayerOwner().ObjectDetail )
	{
		case ObjectDetailVeryLow:	ObjectDetailSlider.SetValue( 0 ); break;
		case ObjectDetailLow:		ObjectDetailSlider.SetValue( 1 ); break;
		case ObjectDetailMedium:	ObjectDetailSlider.SetValue( 2 ); break;
		case ObjectDetailHigh:		ObjectDetailSlider.SetValue( 3 ); break;
		case ObjectDetailVeryHigh:	ObjectDetailSlider.SetValue( 4 ); break;
	}

	bInitialized = true;
}



//-----------------------------------------------------------------------------------------------

function SettingsChanged()
{
	local string NewSettings;

	if(bInitialized)
	{
		OldSettings = GetPlayerOwner().ConsoleCommand("GetCurrentRes")$"x"$GetPlayerOwner().ConsoleCommand("GetCurrentColorDepth");
		NewSettings = ResolutionCombo.GetValue()$"x"$ColorDepthCombo.GetValue2();

		if(NewSettings != OldSettings)
		{
			log("Screen Settings Changed");
			GetPlayerOwner().ConsoleCommand("SetRes "$NewSettings);

			LoadAvailableSettings();
			ConfirmSettings = doHPMessageBox(ConfirmSettingsText, 
						GetLocalFEString("Main_Menu_0001"),// "Yes"
						GetLocalFEString("Main_Menu_0009"), //"No"
				20);
		}
	}
}

function WindowDone (UWindowWindow W)
{
	if(W == ConfirmSettings)
	{
		if(ConfirmSettings.Result == "" ||
		   ConfirmSettings.Result == GetLocalFEString("Main_Menu_0009")) // "No"
		{
			if (ConfirmSettings.bClosedFromTick)
			{
				// Sto: If this fn is inside the tick of Paint, before or afterPaint messages,
				// then do not call setRes directly, as crashes the software renderer.

				hpconsole(root.console).ResTimeOutSettings = OldSettings;
			}
			else
				GetPlayerOwner().ConsoleCommand("SetRes "$OldSettings);

			LoadAvailableSettings();			
			doHPMessageBox(ConfirmSettingsCancelText, "Okay");
		}
		ConfirmSettings = None;
	}
}


function HideWindow()
{
	Super.HideWindow();
	ResolutionCombo.CloseUpWithNoSound();
	ColorDepthCombo.CloseUpWithNoSound();
	TextureDetailCombo.CloseUpWithNoSound();

	GetPlayerOwner().SaveConfig();
}

function BrightnessChanged()
{
	if(bInitialized)
	{
//		BrightnessSlider.SetText(BrightnessText $" - " $int(BrightnessSlider.Value) );

		GetPlayerOwner().ConsoleCommand("set ini:Engine.Engine.ViewportManager Brightness "$(BrightnessSlider.Value / 10));
		GetPlayerOwner().ConsoleCommand("FLUSH");
	}
}

function TextureDetailChanged()
{
	if(bInitialized)
	{
		GetPlayerOwner().ConsoleCommand("set ini:Engine.Engine.ViewportManager TextureDetail "$TextureDetailCombo.GetValue2());
		OldTextureDetail = TextureDetailCombo.GetSelectedIndex();
	}
}

function ObjectDetailChanged ()
{
	// update the detail slider
	switch( ObjectDetailSlider.GetValue() )
	{
		case 0: GetPlayerOwner().ObjectDetail = ObjectDetailVeryLow;	break;
		case 1: GetPlayerOwner().ObjectDetail = ObjectDetailLow;		break;
		case 2: GetPlayerOwner().ObjectDetail = ObjectDetailMedium;		break;
		case 3: GetPlayerOwner().ObjectDetail = ObjectDetailHigh;		break;
		case 4: GetPlayerOwner().ObjectDetail = ObjectDetailVeryHigh;	break;
	}
	GetPlayerOwner().ConsoleCommand("set ini:HGame.Harry ObjectDetail " $GetPlayerOwner().ObjectDetail );
}

function MusicVolumeChanged()
{
	MusicVolumeLabel.SetText( MusicVolumeText $" - " $int(MusicVolumeSlider.Value) );
	GetPlayerOwner().ConsoleCommand("set ini:Engine.Engine.AudioDevice MusicVolume "$(MusicVolumeSlider.Value/100) );
}

function SoundVolumeChanged()
{
	SoundVolumeLabel.SetText( SoundVolumeText $" - " $int(SoundVolumeSlider.Value) );
	GetPlayerOwner().ConsoleCommand("set ini:Engine.Engine.AudioDevice SoundVolume "$(SoundVolumeSlider.Value/100) );
}


function Notify(UWindowDialogControl C, byte E)
{
	local int i;
	
	Super.Notify(C, E);
	
	switch(E)
	{
	case DE_Change:
		switch(C)
		{
		case ResolutionCombo:
		case ColorDepthCombo:
			SettingsChanged();
			break;
			
		case BrightnessSlider:
			BrightnessChanged();
			break;
			
		case TextureDetailCombo:
			TextureDetailChanged();
			break;
		
		case ObjectDetailSlider:
			ObjectDetailChanged();
			break;
			
		case MusicVolumeSlider:
			MusicVolumeChanged();
			break;
			
		case SoundVolumeSlider:
			SoundVolumeChanged();
			break;
			
		}
		break;
	
	case DE_Click:
		switch( C )
		{
			case BackPageButton:
				FEBook(book).DoEscapeFromPage();
				return;
		}
		break;
	}
}


defaultproperties
{
	GameRenderDriver="SoftDrv.SoftwareRenderDevice"

	videoText="Video"
	ResolutionText="Resolution"
	ColorDepthText="Color Depth"
	ConfirmSettingsTitle="Confirm Video Settings Change"
	ConfirmSettingsText="Are you sure you wish to keep these new video settings?"
	ConfirmSettingsCancelTitle="Video Settings Change"
	ConfirmSettingsCancelText="Your previous video settings have been restored."
	BitsText="bit"
	TextureDetailText="Texture Detail"
	ObjectDetailText="Object Detail"
	audioText="Audio"
	MusicVolumeText="Music Volume"
	SoundVolumeText="Sound Volume"

	DetailLevel(0)="Very High"
	DetailLevel(1)="High"
	DetailLevel(2)="Medium"
	DetailLevel(3)="Low"
	DetailLevel(4)="Very Low"

	optionsText="OPTIONS"
	
	LabelTextColor=(R=40,G=180,B=40)
	GoupLabelTextColor=(R=255,G=255,B=255)
	ButtonTextColor=(R=200,G=200,B=200)

	vertSpacing(0)=40
	vertSpacing(1)=40
	vertSpacing(2)=40
	vertSpacing(3)=40
	vertSpacing(4)=40
	vertSpacing(5)=40
	vertSpacing(6)=40
	vertSpacing(7)=40
}
