class FEInputPage expands baseFEPage;

#EXEC TEXTURE IMPORT NAME=FEOverOptionTexture	 FILE=TEXTURES\Menu\Options\overoption.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEOverOption3Texture	 FILE=TEXTURES\Menu\Options\overoption3.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=FEOverOption5Texture	 FILE=TEXTURES\Menu\Options\overoption5.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


var bool					bInitialized;
var string					OldSettings;

// Mouse label
var UWindowButton			OptionsLabel;
var localized string		optionsText;
							

var UWindowLabelControl		InGame_Label;
var localized string		InGame_Text;

var UWindowLabelControl		MiscLabel;
var localized string		MiscText;

var UWindowLabelControl		InQuidditch_Label;
var localized string		InQuidditch_Text;

var UWindowLabelControl		InWizardDuel_Label;
var localized string		InWizardDuel_Text;

var UWindowCheckbox			AutoCenterCamCheck;
var localized string		AutoCenterCamText;

var UWindowCheckbox			AutoDrinkPotionCheck;
var localized string		AutoDrinkPotionText;

var UWindowCheckbox			MoveWhileCastingCheck;
var localized string		MoveWhileCastingText;


// Mouse Sensitivity
var UWindowHSliderControl	SensitivitySlider;

// Mouse Invert
var UWindowCheckbox			MouseInvertCheck;
var localized string		MouseInvertText;

// Controls Label
//var UWindowLabelControl		ControlLabel;
//var localized string		controlText;

// Mouse labels
var UWindowLabelControl		MouseSensitivityLabel;
var localized string		MouseSensitivityText;							

var UWindowLabelControl		MouseHiLabel, MouseLoLabel;
var localized string		MouseHiText, MouseLoText;

// Mouse Sensitivity
var UWindowEditControl		SensitivityEdit;
var localized string		SensitivityText;

var UWindowCheckbox			MouseSmoothCheck;
var localized string		MouseSmoothText;

var HPMessageBox			ConfirmSettings;

var localized string		ConfirmSettingsTitle;
var localized string		ConfirmSettingsText;
var localized string		ConfirmSettingsCancelTitle;
var localized string		ConfirmSettingsCancelText;

var UWindowLabelControl		KeyboardLabel;
var localized string		KeyboardText;



// Keyboard settings
//------------------
var string					RealKeyName[255];
var localized string		LocalizedKeyName[255];


var localized string		OrString;


var UWindowButton			SelectedButton;
var bool					bPolling;
var int						Selection;

var string					AliasNames1[13];
var string					AliasNames2[13];
var string					AliasNames3[13];
var int						BoundKey1[13];
var int						BoundKey2[13];

// You can set these RemoveExistingBoundKey min/max vars when setting 
// a key to have more control over what BoundKeys should be overridden.
var int						RemoveExistingBoundKeyMinIndex;
var int						RemoveExistingBoundKeyMaxIndex;

var enum EKeyType
{
	KT_Game,
	KT_Quidditch,
	KT_WizardDuel,

} SetKeyType;

// *** KEY BUTTON GROUPS ***
// --- InGame input group ( 9 keys )
const						InGame_FirstBoundKeyIndex = 0;
const						InGame_LastBoundKeyIndex  = 9;
var localized string		InGame_LabelList[9];
var UWindowLabelControl		InGame_KeyNames[9];
var HPMenuRaisedButton		InGame_KeyButtons[9];
var int						InGame_VertSpacing[9];


// --- Quidditch input group ( 2 keys )
const						InQuidditch_FirstBoundKeyIndex = 9;
const						InQuidditch_LastBoundKeyIndex  = 11;
var localized string		InQuidditch_LabelList[2];
var UWindowLabelControl		InQuidditch_KeyNames[2];
var HPMenuRaisedButton		InQuidditch_KeyButtons[2];
var int						InQuidditch_VertSpacing[2];


// --- Wizard Dueling input group ( 1 key )
const						InWizardDuel_FirstBoundKeyIndex = 11;
const						InWizardDuel_LastBoundKeyIndex  = 12;
var localized string		InWizardDuel_LabelList[2];				// <-- Even though there is one key we need an array of 2 to use this var as an array...
var UWindowLabelControl		InWizardDuel_KeyNames[2];
var HPMenuRaisedButton		InWizardDuel_KeyButtons[2];
var int						InWizardDuel_VertSpacing[2];

// auto jump control		
var UWindowCheckbox			AutoJumpCheck;
var localized string		AutoJumpText;


// --- Quidditch

// invert flying control	
var UWindowCheckbox			InvertBroomCheck;
var localized string		InvertBroomText;


// Difficulty
var UWindowLabelControl		DifficultyLabel;
var HPMenuOptionCombo		DifficultyCombo;
var localized string		DifficultyText;
var localized string		DifficultyLevel[3];

var Color					LabelTextColor, GoupLabelTextColor, ButtonTextColor;

var sound					buttonClickSound;


function LocalizeStrings()
{
	local int i;
	local string tmpStr;
	
	// options_16	In Game
	InGame_Text=GetLocalFEString("Options_0036");	//"InGame"
	
	// options_17	Mouse Speed
	MouseSensitivityText=GetLocalFEString("Options_0013");	//"Mouse Speed" / "Mouse Sensitivity"

	// options_07	High
	MouseHiText=GetLocalFEString("Options_0002");	//"High"
	// options_10	Low
	MouseLoText=GetLocalFEString("Options_0005");	//"Low"
	
	
	// --- In Game Key Labels
	// options_21	Forward
	InGame_LabelList[0]=GetLocalFEString("Options_0032");	//"Forward"
	
	// options_22	Backward
	InGame_LabelList[1]=GetLocalFEString("Options_0008");	//"Backward"
	
	// options_23	Turn Left
	InGame_LabelList[2]=GetLocalFEString("Options_0031");	//"Turn left"
	
	// options_24	Turn InGame
	InGame_LabelList[3]=GetLocalFEString("Options_0030");	//"Turn right"
	
	// options_25	Jump
	InGame_LabelList[4]=GetLocalFEString("Options_0029");	//"Jump"
	
	// options_26	Use Wand
	InGame_LabelList[5]=GetLocalFEString("Options_0017");	//"Use Wand" / "Fire Wand"
	
	// options_26	Drink Potion
	InGame_LabelList[6]=GetLocalFEString("InGameMenu_0033");	//"Drink Potion"
	
	// flying_02	Strafe Left
	InGame_LabelList[7]=GetLocalFEString("Flying_0004");	//"Strafe Left"
	
	// flying_03	Strafe InGame
	InGame_LabelList[8]=GetLocalFEString("Flying_0005");	//"Strafe Right"
	
	// ------------------------------------------------------
	// --- In Quidditch Key Labels
	// flying_02	Speed Up
//	InQuidditch_LabelList[9]=GetLocalFEString("Flying_0001");	//"Speed up" // broom
	
	// flying_03	Slow Down
//	InQuidditch_LabelList[10]=GetLocalFEString("Flying_0002");	//"Slow down" // broom

	// ------------------------------------------------------
	// --- In Wizard Dueling Key Labels
	// Cycle Current Spell
//	InGame_LabelList[11]=GetLocalFEString("Flying_0001");	//"Cycle Spell" // broom
	
	// options_20	Keyboard
	KeyboardText=GetLocalFEString("Options_0018");	//"Keyboard"
	
	// --- Flying controls
	
	// flying_04	Invert Broom Control
	InvertBroomText=GetLocalFEString("Flying_0003");//"Invert Broom"
	
	// options_01	Page Title
	optionsText = GetLocalFEString("Options_0040");	//"Input Options"
	
	for(i=0;i<255;i++)
	{
		// build number part. I wish i could do a sprintf("%03d");
		tmpStr="0000" $i;
		tmpStr="Localized_kn_" $right(tmpStr,4);
		LocalizedKeyName[i]=localize("all", tmpStr,"HPMenu");
//		log("Parse:"$tmpStr $" " $LocalizedKeyName[i]);
	}

/***************Not Used**************************/
	MouseSmoothText="Mouse Smoothing";
/***************Not Used**************************/
	
	MiscText		  = GetLocalFEString("Options_0042");
	InGame_Text		  = GetLocalFEString("Options_0036");
	InQuidditch_Text  = GetLocalFEString("Options_0037");
	InWizardDuel_Text = GetLocalFEString("Options_0038");
	
	OrString		  = " " $GetLocalFEString("Options_0045") $" ";	// " or "
	AutoJumpText	  = GetLocalFEString("Options_0046");			// "Auto Jump"
	
	MouseInvertText		 = GetLocalFEString("Options_0035");		// "Invert Mouse"
	MoveWhileCastingText = GetLocalFEString("Options_0043");		// "Move While Casting"
	AutoCenterCamText	 = GetLocalFEString("InGameMenu_0032");		// "Auto center camera"
	AutoDrinkPotionText	 = GetLocalFEString("InGameMenu_0034");		// "Auto drink potion"
	
	ConfirmSettingsTitle		= GetLocalFEString("Options_0047");	// "Confirm Video Settings Change";
	ConfirmSettingsText			= GetLocalFEString("Options_0048");	// "Are you sure you wish to keep these new video settings?";
	ConfirmSettingsCancelTitle	= GetLocalFEString("Options_0049");	// "Video Settings Change";
	ConfirmSettingsCancelText	= GetLocalFEString("Options_0050");	// "Your previous video settings have been restored.";
	
	DifficultyText	   = GetLocalFEString("Options_0052");
	DifficultyLevel[0] = GetLocalFEString("Options_0053");
	DifficultyLevel[1] = GetLocalFEString("Options_0054");
	DifficultyLevel[2] = GetLocalFEString("Options_0055");
}


function Created()
{
	local int ctlX, ctlY, ctlW, ctlH, labelWidth, labelX, offsetX, offsetY, I;
	local int MusicVolume, SoundVolume;
	local string sens;

	LocalizeStrings();

	LoadExistingKeys();

/*	OptionsLabel = UWindowButton(CreateControl(class'UWindowButton', 264-140, 17, 280, 25));
	OptionsLabel.SetText(optionsText);
	OptionsLabel.Align=TA_Center;
	OptionsLabel.SetFont(F_HPMenuLarge);
	OptionsLabel.TextColor = LabelTextColor;
	OptionsLabel.ShowWindow();
*/
	offsetX = 0;
	offsetY = 0;

	// Set to Left-hand of page
	//-----------------------

	ctlX   = 170-offsetX;
	labelX = ctlX-80;
	ctlY = 50-offsetY;
	ctlH = 17;
	ctlW = 134;
	labelWidth = ctlX-labelX;


	MiscLabel = UWindowLabelControl(CreateControl(class'UWindowLabelControl', labelX, ctlY, ctlW, 1));
	MiscLabel.SetText(MiscText);
	MiscLabel.SetFont(F_Bold);
	MiscLabel.TextColor = GoupLabelTextColor;
	ctlY += 25;

	I = 0;
	
	// Mouse Sensitivity
	MouseSensitivityLabel = UWindowLabelControl(CreateControl(class'UWindowLabelControl', labelX, ctlY+10, ctlW, 1));
	MouseSensitivityLabel.SetText( MouseSensitivityText );
	MouseSensitivityLabel.SetFont(F_Bold);
	MouseSensitivityLabel.TextColor = LabelTextColor;
	
	SensitivitySlider = HPMenuOptionHSlider(CreateControl(class'HPMenuOptionHSlider', ctlX, ctlY, ctlW, 1));
	SensitivitySlider.bNoSlidingNotify = True;
	SensitivitySlider.SetRange(0.2, 10, 0.2);
	ctlY += 28;

	MouseHiLabel = UWindowLabelControl(CreateControl(class'UWindowLabelControl', ctlX+115, ctlY, ctlW, 1));
	MouseHiLabel.SetText(MouseHiText);
	MouseHiLabel.SetFont(F_Normal);
	MouseHiLabel.TextColor = ButtonTextColor;

	MouseLoLabel = UWindowLabelControl(CreateControl(class'UWindowLabelControl', ctlX, ctlY, ctlW, 1));
	MouseLoLabel.SetText(MouseLoText);
	MouseLoLabel.SetFont(F_Normal);
	MouseLoLabel.TextColor = ButtonTextColor;
	ctlY += 20;
	
	// Mouse Invert CheckBox
	MouseInvertCheck = HPMenuOptionCheckBox(CreateControl(class'HPMenuOptionCheckBox', ctlX, ctlY, 160, 1));
	MouseInvertCheck.bChecked = Harry(GetPlayerOwner()).bInvertMouse;
	MouseInvertCheck.SetText( MouseInvertText );
	MouseInvertCheck.SetFont( F_Normal );
	MouseInvertCheck.TextColor = LabelTextColor;
	ctlY += 20;
	log("ini:Engine.PlayerPawn bInvertMouse -> " $MouseInvertCheck.bChecked );

	// Move while casting
	MoveWhileCastingCheck = HPMenuOptionCheckBox(CreateControl(class'HPMenuOptionCheckBox', ctlX, ctlY, 160, 1));
	MoveWhileCastingCheck.bChecked = Harry(GetPlayerOwner()).bMoveWhileCasting;
	MoveWhileCastingCheck.SetText(MoveWhileCastingText);
	MoveWhileCastingCheck.SetFont(F_Normal);
	MoveWhileCastingCheck.TextColor = LabelTextColor;
	ctlY += 20;
	log("get ini:HGame.Harry bMoveWhileCasting -> " $MoveWhileCastingCheck.bChecked );	
	
	// Auto Center Camera CheckBox
	AutoCenterCamCheck = HPMenuOptionCheckBox(CreateControl(class'HPMenuOptionCheckBox', ctlX, ctlY, 160, 1));
	AutoCenterCamCheck.bChecked = Harry(GetPlayerOwner()).bAutoCenterCamera;
	AutoCenterCamCheck.SetText( AutoCenterCamText );
	AutoCenterCamCheck.SetFont( F_Normal );
	AutoCenterCamCheck.TextColor = LabelTextColor;
	ctlY += 20;
	log("get ini:HGame.Harry bAutoCenterCamera -> " $AutoCenterCamCheck.bChecked );
	
	// Auto Jump checkbox
	AutoJumpCheck = HPMenuOptionCheckBox(CreateControl(class'HPMenuOptionCheckBox', ctlX, ctlY, 160, 1));
	AutoJumpCheck.bChecked = Harry(GetPlayerOwner()).bAutoJump;
	AutoJumpCheck.SetText(AutoJumpText);
	AutoJumpCheck.SetFont(F_Normal);
	AutoJumpCheck.TextColor = LabelTextColor;
	ctlY += 20;
	log("AutoJumpCheck -> " $AutoJumpCheck.bChecked );

	// Auto Drink Potion CheckBox
	AutoDrinkPotionCheck = HPMenuOptionCheckBox(CreateControl(class'HPMenuOptionCheckBox', ctlX, ctlY, 160, 1));
	AutoDrinkPotionCheck.bChecked = Harry(GetPlayerOwner()).bAutoQuaff;
	AutoDrinkPotionCheck.SetText( AutoDrinkPotionText );
	AutoDrinkPotionCheck.SetFont( F_Normal );
	AutoDrinkPotionCheck.TextColor = LabelTextColor;
	ctlY += 25;
	log("get ini:HGame.Harry bAutoQuaff -> " $AutoDrinkPotionCheck.bChecked );

	// Game Difficulty
	DifficultyLabel = UWindowLabelControl(CreateControl(class'UWindowLabelControl', labelX, ctlY+3, ctlW, 1));
	DifficultyLabel.SetText( DifficultyText );
	DifficultyLabel.SetFont(F_Bold);
	DifficultyLabel.TextColor = LabelTextColor;
	
	DifficultyCombo = HPMenuOptionCombo(CreateControl(class'HPMenuOptionCombo', ctlX, ctlY, ctlW, 1));
	DifficultyCombo.SetEditable( False );
	DifficultyCombo.EditBoxWidth = ctlW;
	DifficultyCombo.TextColor    = LabelTextColor;
	DifficultyCombo.SetEditTextColor( ButtonTextColor );
	
	// The display names are localized.
	DifficultyCombo.AddItem( DifficultyLevel[0], "DifficultyEasy" );
	DifficultyCombo.AddItem( DifficultyLevel[1], "DifficultyMedium" );
	DifficultyCombo.AddItem( DifficultyLevel[2], "DifficultyHard" );
	ctlY += 35;

	//*******************
	// --- Quidditch keys
	InQuidditch_Label = UWindowLabelControl(CreateControl(class'UWindowLabelControl', labelX, ctlY, ctlW, 1));
	InQuidditch_Label.SetText(InQuidditch_Text);
	InQuidditch_Label.SetFont(F_Bold);
	InQuidditch_Label.TextColor = GoupLabelTextColor;
	ctlY += 20;
	

	// Invert broom checkbox
	InvertBroomCheck = HPMenuOptionCheckBox(CreateControl(class'HPMenuOptionCheckBox', ctlX, ctlY, 160, 1));
	InvertBroomCheck.bChecked = IsFlyingControlInverted ();
	InvertBroomCheck.SetText(InvertBroomText);
	InvertBroomCheck.SetFont(F_Normal);
	InvertBroomCheck.TextColor = LabelTextColor;
	ctlY += InQuidditch_VertSpacing[0];

	// In Quidditch keyboard settings
/*	for (I=0; I<ArrayCount(InQuidditch_LabelList); I++)
	{
		InQuidditch_KeyNames[I] = UWindowLabelControl(CreateControl(class'UWindowLabelControl', labelX, ctlY+3, labelWidth+200, 1));
		InQuidditch_KeyNames[I].SetText(InQuidditch_LabelList[I]);
		InQuidditch_KeyNames[I].SetFont(F_Bold);
		InQuidditch_KeyNames[I].TextColor = LabelTextColor;
		
		InQuidditch_KeyButtons[I] = HPMenuRaisedButton(CreateControl(class'HPMenuRaisedButton', ctlX, ctlY, ctlW, ctlH));
		InQuidditch_KeyButtons[I].UpTexture				= Texture'FEOverOption5Texture';
		InQuidditch_KeyButtons[I].DownTexture			= Texture'FEOverOptionTexture';
		InQuidditch_KeyButtons[I].DisabledTexture		= Texture'FEOverOptionTexture';
		InQuidditch_KeyButtons[I].OverTexture			= Texture'FEOverOption3Texture';
		InQuidditch_KeyButtons[I].SetFont(F_Normal);
		InQuidditch_KeyButtons[I].bAcceptsFocus			= False;
		InQuidditch_KeyButtons[I].bIgnoreLDoubleClick	= True;
		InQuidditch_KeyButtons[I].bIgnoreMDoubleClick	= True;
		InQuidditch_KeyButtons[I].bIgnoreRDoubleClick	= True;
		InQuidditch_KeyButtons[I].TextColor				= ButtonTextColor;
		
		ctlY += InQuidditch_VertSpacing[I];
	}
*/	
	ctlY += 13;

		//*******************
	// --- Wizard Duel keys

	// In WizardDuel label
	InWizardDuel_Label = UWindowLabelControl(CreateControl(class'UWindowLabelControl', labelX, ctlY, ctlW, 1));
	InWizardDuel_Label.SetText(InWizardDuel_Text);
	InWizardDuel_Label.SetFont(F_Bold);
	InWizardDuel_Label.TextColor = GoupLabelTextColor;
	ctlY += 25;
	
	// In Wizard Duel keyboard settings
	for (I=0; I<1/*ArrayCount(InWizardDuel_LabelList)*/; I++)
	{
		InWizardDuel_KeyNames[I] = UWindowLabelControl(CreateControl(class'UWindowLabelControl', labelX, ctlY+3, labelWidth, 1));
		InWizardDuel_KeyNames[I].SetText(InWizardDuel_LabelList[I]);
		InWizardDuel_KeyNames[I].SetFont(F_Bold);
		InWizardDuel_KeyNames[I].TextColor = LabelTextColor;
		
		InWizardDuel_KeyButtons[I] = HPMenuRaisedButton(CreateControl(class'HPMenuRaisedButton', ctlX, ctlY, ctlW, ctlH));
		InWizardDuel_KeyButtons[I].UpTexture			= Texture'FEOverOption5Texture';
		InWizardDuel_KeyButtons[I].DownTexture			= Texture'FEOverOptionTexture';
		InWizardDuel_KeyButtons[I].DisabledTexture		= Texture'FEOverOptionTexture';
		InWizardDuel_KeyButtons[I].OverTexture			= Texture'FEOverOption3Texture';
		InWizardDuel_KeyButtons[I].SetFont(F_Normal);
		InWizardDuel_KeyButtons[I].bAcceptsFocus		= False;
		InWizardDuel_KeyButtons[I].bIgnoreLDoubleClick	= True;
		InWizardDuel_KeyButtons[I].bIgnoreMDoubleClick	= True;
		InWizardDuel_KeyButtons[I].bIgnoreRDoubleClick	= True;
		InWizardDuel_KeyButtons[I].TextColor			= ButtonTextColor;

		ctlY += InWizardDuel_VertSpacing[I];
	}
	
	// ---------------------------------------------------------------------------------------------------------------
	// Set to Right-hand page
	//-----------------------
	ctlY   = 50 -offsetY;
	ctlX   = 420-offsetX; //350-offsetX;
	labelX = 350-offsetX; //485-offsetX;


	//*******************
	// InGame keyboard settings

	InGame_Label = UWindowLabelControl(CreateControl(class'UWindowLabelControl', labelX, ctlY, ctlW, 1));
	InGame_Label.SetText(InGame_Text);
	InGame_Label.SetFont(F_Bold);
	InGame_Label.TextColor = GoupLabelTextColor;
	ctlY += 25;

	for (I=0; I<ArrayCount(InGame_LabelList); I++)
	{
		InGame_KeyNames[I] = UWindowLabelControl(CreateControl(class'UWindowLabelControl', labelX, ctlY+3, labelWidth, 1));
		InGame_KeyNames[I].SetText(InGame_LabelList[I]);
		InGame_KeyNames[I].SetFont(F_Bold);
		InGame_KeyNames[I].TextColor = LabelTextColor;


		InGame_KeyButtons[I] = HPMenuRaisedButton(CreateControl(class'HPMenuRaisedButton', ctlX, ctlY, ctlW, ctlH));
		InGame_KeyButtons[I].UpTexture				= Texture'FEOverOption5Texture';
		InGame_KeyButtons[I].DownTexture			= Texture'FEOverOptionTexture';
		InGame_KeyButtons[I].DisabledTexture		= Texture'FEOverOptionTexture';
		InGame_KeyButtons[I].OverTexture			= Texture'FEOverOption3Texture';
		InGame_KeyButtons[I].SetFont(F_Normal);
		InGame_KeyButtons[I].bAcceptsFocus			= False;
		InGame_KeyButtons[I].bIgnoreLDoubleClick	= True;
		InGame_KeyButtons[I].bIgnoreMDoubleClick	= True;
		InGame_KeyButtons[I].bIgnoreRDoubleClick	= True;
		InGame_KeyButtons[I].TextColor				= ButtonTextColor;

		ctlY += InGame_VertSpacing[I];
	}

	

	// ----------------------------------------------------------------------

	LoadAvailableSettings();

	// Create our BackPage button
	CreateBackPageButton();
}

function HideWindow()
{
	Super.HideWindow();
	
	DifficultyCombo.CloseUpWithNoSound();
	GetPlayerOwner().SaveConfig();
}

function PlayClick()
{
    if( buttonClickSound != None )
    {
        GetPlayerOwner().PlaySound( buttonClickSound, SLOT_Interact );
    }
}
 



function BeforePaint(Canvas C, float X, float Y)
{
	local int I, J;

	for (I=0; I<ArrayCount(InGame_KeyButtons); I++ )
	{
		if ( BoundKey1[I] == 0 )
			InGame_KeyButtons[I].SetText("");
		else
		if ( BoundKey2[I] == 0 )
			InGame_KeyButtons[I].SetText(LocalizedKeyName[BoundKey1[I]]);
		else
			InGame_KeyButtons[I].SetText(LocalizedKeyName[BoundKey1[I]]$OrString$LocalizedKeyName[BoundKey2[I]]);
	}

	for (I=0; I<ArrayCount(InQuidditch_KeyButtons); I++ )
	{
		J = InQuidditch_FirstBoundKeyIndex+I;
		if ( BoundKey1[J] == 0 )
			InQuidditch_KeyButtons[I].SetText("");
		else
		if ( BoundKey2[J] == 0 )
			InQuidditch_KeyButtons[I].SetText(LocalizedKeyName[BoundKey1[J]]);
		else
			InQuidditch_KeyButtons[I].SetText(LocalizedKeyName[BoundKey1[J]]$OrString$LocalizedKeyName[BoundKey2[J]]);
	}

	for (I=0; I<1; I++ ) //ArrayCount(InWizardDuel_KeyButtons)
	{
		J = InWizardDuel_FirstBoundKeyIndex+I;
		
		if ( BoundKey1[J] == 0 )
			InWizardDuel_KeyButtons[I].SetText("");
		else
		if ( BoundKey2[J] == 0 )
			InWizardDuel_KeyButtons[I].SetText(LocalizedKeyName[BoundKey1[J]]);
		else
			InWizardDuel_KeyButtons[I].SetText(LocalizedKeyName[BoundKey1[J]]$OrString$LocalizedKeyName[BoundKey2[J]]);
	}

}



function LoadAvailableSettings()
{
	local string ParseString;
	local int P, I;
	local string TempStr;

	bInitialized = false;

	// Load settings here
	
	DifficultyCombo.SetSelectedIndex( GetPlayerOwner().Difficulty );
	
	SensitivitySlider.SetValue( GetPlayerOwner().MouseSensitivity );

	bInitialized = true;
}

function SwitchFlyingControlAliases ()
{
	local string tmp;
	tmp = AliasNames2[0];
	AliasNames2[0] = AliasNames2[1];
	AliasNames2[1] = tmp;
	
	log("Flying control aliases switched");
}

function bool IsFlyingControlInverted ()
{
/*	if (AliasNames2[0] ~= "button bBroomPitchUp")
		return false;
	else
		return true;
*/
	return Harry(GetPlayerOwner()).bInvertBroomPitch;
}



function LoadExistingKeys()
{
	local int I, J, pos;
	local string KeyName;
	local string Alias;

	// capitalise all aliases to simplify comparison code
	for (J=0; J<ArrayCount(AliasNames1); J++)
	{
		AliasNames1[J] = Caps(AliasNames1[J]);
		AliasNames2[J] = Caps(AliasNames2[J]);
		AliasNames3[J] = Caps(AliasNames3[J]);
	}


	for (I=0; I<ArrayCount(BoundKey1); I++)
	{
		BoundKey1[I] = 0;
		BoundKey2[I] = 0;
	}

	for (I=0; I<255; I++)
	{
		KeyName = GetPlayerOwner().ConsoleCommand( "KEYNAME "$i );

		RealKeyName[i] = KeyName;
		if( KeyName != "" )
		{
			Alias = GetPlayerOwner().ConsoleCommand( "KEYBINDING "$KeyName );
			if( Alias != "" )
			{
//				log("OptionPage keyname "$I $" is " $keyname $", KeyBinding Alias is " $Alias);

				Alias  = Caps(Alias);

				// Are any of the aliases that we use stored in this alias string ?
				for( J=0; J<ArrayCount(AliasNames1); J++ )
				{
					if( AliasNames1[J] != "" &&
						InStr( Alias, Caps( AliasNames1[J]) ) != -1 )
					{
						if( BoundKey1[J] == 0 )	    BoundKey1[J] = I;
						else if( BoundKey2[J] == 0) BoundKey2[J] = I;
						log(" Found in AliasNames1[" $J $"] (" $AliasNames1[J] $") set to BoundKey[" $J $"] " );
					//	break;
					}
				}//endfor
			}
		}
	}//endfor
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


function SensitivityChanged()
{
	GetPlayerOwner().MouseSensitivity = SensitivitySlider.Value;

	log("Sensitivity changed to " $GetPlayerOwner().MouseSensitivity );
}

function MouseSmoothChanged()
{
	GetPlayerOwner().bMaxMouseSmoothing = MouseSmoothCheck.bChecked;
	log("bMaxMouseSmoothing changed to " $MouseSmoothCheck.bChecked );
}

function MouseInvertChanged()
{
	GetPlayerOwner().InvertMouse( MouseInvertCheck.bChecked );
	GetPlayerOwner().ConsoleCommand("set ini:Engine.PlayerPawn bInvertMouse "$AutoCenterCamCheck.bChecked );
	log("MouseInvert changed to " $MouseInvertCheck.bChecked );
}

function AutoCenterCamChanged()
{
	Harry(GetPlayerOwner()).bAutoCenterCamera = AutoCenterCamCheck.bChecked;
	GetPlayerOwner().ConsoleCommand("set ini:HGame.Harry bAutoCenterCamera "$AutoCenterCamCheck.bChecked );
	log("AutoCenterCam changed to " $AutoCenterCamCheck.bChecked );
}

function AutoDrinkPotionChanged()
{
	//
	Harry(GetPlayerOwner()).bAutoQuaff = AutoDrinkPotionCheck.bChecked;
	GetPlayerOwner().ConsoleCommand("set ini:HGame.Harry bAutoQuaff "$AutoDrinkPotionCheck.bChecked );
	log("bAutoQuaff changed to " $AutoDrinkPotionCheck.bChecked );
}

function MoveWhileCastingChanged()
{
	Harry(GetPlayerOwner()).bMoveWhileCasting = MoveWhileCastingCheck.bChecked;
	GetPlayerOwner().ConsoleCommand("set ini:HGame.Harry bMoveWhileCasting "$MoveWhileCastingCheck.bChecked );
	log("MoveWhileCasting changed to " $MoveWhileCastingCheck.bChecked );
}



function AutoJumpChanged()
{
	GetPlayerOwner().AutoJump( AutoJumpCheck.bChecked );
	log("AutoJumpChanged changed to " $AutoJumpCheck.bChecked );
}


function InvertBroomChanged ()
{
/*	SwitchFlyingControlAliases();

	if (BoundKey1[0] != 0)
		GetPlayerOwner().ConsoleCommand("SET Input"@RealKeyName[BoundKey1[0]]@AliasNames1[0]@"|"@AliasNames2[0]);		
	if (BoundKey1[1] != 0)
		GetPlayerOwner().ConsoleCommand("SET Input"@RealKeyName[BoundKey1[1]]@AliasNames1[1]@"|"@AliasNames2[1]);		

	if (BoundKey2[0] != 0)
		GetPlayerOwner().ConsoleCommand("SET Input"@RealKeyName[BoundKey2[0]]@AliasNames1[0]@"|"@AliasNames2[0]);		
	if (BoundKey2[1] != 0)
		GetPlayerOwner().ConsoleCommand("SET Input"@RealKeyName[BoundKey2[1]]@AliasNames1[1]@"|"@AliasNames2[1]);		
*/

//	Harry(GetPlayerOwner()).bInvertBroomPitch = InvertBroomCheck.bChecked;

    // Changed from a direct variable access to a function call.
    Harry(GetPlayerOwner()).InvertBroomPitch(InvertBroomCheck.bChecked);

}


function DifficultyChanged()
{
	local string str;
	
	switch (DifficultyCombo.GetSelectedIndex())
	{
		case 0:  GetPlayerOwner().Difficulty = DifficultyEasy;	 str = "DifficultyEasy";	break;
		case 1:  GetPlayerOwner().Difficulty = DifficultyMedium; str = "DifficultyMedium";	break;
		case 2:  GetPlayerOwner().Difficulty = DifficultyHard;	 str = "DifficultyHard";	break;
		
		default: GetPlayerOwner().Difficulty = DifficultyMedium; str = "DifficultyMedium";	break;
	}
	
	GetPlayerOwner().ConsoleCommand("set ini:HGame.Harry Difficulty " $str );
	log("Difficulty changed to " $GetPlayerOwner().Difficulty );
}


function RemoveExistingKey( int KeyNo, string KeyName )
{
	local int I;

	// Remove this key from any existing binding display
	for ( I=RemoveExistingBoundKeyMinIndex; I<ArrayCount(Boundkey1) && I<RemoveExistingBoundKeyMaxIndex; I++ )
	{
		if(I != Selection)
		{
			if ( BoundKey2[I] == KeyNo )
			{			
				BoundKey2[I] = 0;

				// Remove this key from this action
				log(" Removing Key -" $RealKeyName[KeyNo] $"- with AliasNames1[I] -" $AliasNames1[I] $" 2 -" $AliasNames2[I] $" 3 -" $AliasNames3[I] );
				
				GetPlayerOwner().ConsoleCommand("SET Input "@RealKeyName[KeyNo]@RemoveActionFromKey( RealKeyName[KeyNo], AliasNames1[I] ) );
				if( AliasNames2[I] != "" )
					GetPlayerOwner().ConsoleCommand("SET Input "@RealKeyName[KeyNo]@RemoveActionFromKey( RealKeyName[KeyNo], AliasNames2[I] ) );
				if( AliasNames3[I] != "" )
					GetPlayerOwner().ConsoleCommand("SET Input "@RealKeyName[KeyNo]@RemoveActionFromKey( RealKeyName[KeyNo], AliasNames3[I] ) );
			}
			
			if ( BoundKey1[I] == KeyNo )
			{
				BoundKey1[I] = BoundKey2[I];
				BoundKey2[I] = 0;

				// Remove this key from this action
				log(" Removing Key -" $RealKeyName[KeyNo] $"- with AliasNames1[I] -" $AliasNames1[I] $" 2 -" $AliasNames2[I] $" 3 -" $AliasNames3[I] );

				GetPlayerOwner().ConsoleCommand("SET Input "@RealKeyName[KeyNo]@RemoveActionFromKey( RealKeyName[KeyNo], AliasNames1[I] ) );
				if( AliasNames2[I] != "" )
					GetPlayerOwner().ConsoleCommand("SET Input "@RealKeyName[KeyNo]@RemoveActionFromKey( RealKeyName[KeyNo], AliasNames2[I] ) );
				if( AliasNames3[I] != "" )
					GetPlayerOwner().ConsoleCommand("SET Input "@RealKeyName[KeyNo]@RemoveActionFromKey( RealKeyName[KeyNo], AliasNames3[I] ) );
			}
		}
	}
}

function string AddTokenToString( string InString, string InToken )
{
	local int j;

	InString = Caps( InString );
	InToken  = Caps( InToken );
	
		
	if( InToken == "" )
		return InString;
	
	if( InStr(InString, InToken ) != -1 )
	{
		return InString;
	}

	j = Len( InString );

	// The Key ISN'T in the string so return the string ORed with the Key
	if( j > 0 )
	{
		j--;
		while( j > 0 && (Mid(InString, j, 1) == "|" || Mid(InString, j, 1) == " ") )
		{
			// If this is the only token assigned to this string then don't use OR
			if( Mid(InString, j, 1) == "=" )
			{
				log(" InString["$j$"] == '=' so we will return -" $InString@InToken );
				return (InString@InToken);
			}
			j--;
		}
		log(" InString["$j$"] != '=' so we will return -" $(InString@"|"@InToken) );
		
		return (InString@"|"@InToken);
	}
	else
		return (InToken);
}

function string RemoveTokenFromString( string InString, string InToken )
{
	local string l, r;
	local int i, j;

	if( InString == "" || InToken == "" )
		return InString;
	
	InString = Caps( InString );
	InToken  = Caps( InToken );
	
	// See if we have the key in the string
	i = InStr(InString, InToken );
	if( i != -1 )
	{
		// --- Remove key from string
		log( "Removing Token:" $InToken $" from string:" $InString $"!" );
		
		// Get the left part of the current string
		l = Left( InString, i );
		
		// Get the right part of the current string
		r = Right( InString, Len(InString) - (i+Len(InToken)) );
		log( "InStr returned:" $i $" Len(leftPart):" $Len(l) $" Len(InToken):" $Len(InToken) );
		
		// Start at the end of the left string and cut off all characters that are " " or "|"
		j = Len( l );
		j--;
		while( j > 0 && (Mid(l, j, 1) == "|" || Mid(l, j, 1) == " ") )
		{
			//DEBUG
			log( "j=" $j $"! Mid(l, j, 1)=" $Mid(l, j, 1) $"!l=" $l $"!" );

			// cut off the end char.
			l = Left( l, j--);
		}
		
		//DEBUG
		log("After cleanup left:" $l );
		log("right:" $r );
		log("final:" $l$r );
		

		// make l our final string
		l = l$r;
		
		// Clean up our final string
		while( Len( l ) > 0 && (Mid(l, 0, 1) == "|" || Mid(l, 0, 1) == " ") )
		{
			//DEBUG
			log( "j=" $j $"! Mid(l, 0, 1)=" $Mid(l, 0, 1) $"!l=" $l $"!" );
			
			// cut out first char
			l = Right( l, Len( l )-1);
		}

		// Clean up final string
		log("After cleanup final:" $l );

		// return the new string as the exsisting left and right parts.
		return (l);
	}
	return InString;
}

function string RemoveActionFromKey( string KeyName, string ActionName )
{
	local string keybinding;
	keybinding = GetPlayerOwner().ConsoleCommand( "KEYBINDING "$KeyName );
	log("Calling RemoveActionFromKey(" $keybinding $"," $ActionName$");");
	return RemoveTokenFromString( keybinding, ActionName );
}

function string AddActionToKey( string KeyName, string ActionName )
{
	local string keybinding;
	keybinding = GetPlayerOwner().ConsoleCommand( "KEYBINDING "$KeyName );
	log("Calling AddActionToKey(" $keybinding $"," $ActionName$");");
	return AddTokenToString( keybinding, ActionName );
}

function SetKey(int KeyNo, string KeyName )
{
	local string RemoveKeyName;

	log( "options Setkey"@KeyName $" KeyNo"@KeyNo );

	if ( BoundKey1[Selection] != 0 )
	{
		// if this key is already chosen, just clear out other slot
		if( KeyNo == BoundKey1[Selection] )
		{
			// if 2 exists, remove it it.
			if( BoundKey2[Selection] != 0 )
			{
				// *** Remove key in slot 2
				RemoveKeyName = RealKeyName[BoundKey2[Selection]];
				GetPlayerOwner().ConsoleCommand("SET Input "@RemoveKeyName@RemoveActionFromKey( RemoveKeyName, AliasNames1[Selection] ) );
				if( AliasNames2[Selection] != "" )
					GetPlayerOwner().ConsoleCommand("SET Input "@RemoveKeyName@RemoveActionFromKey( RemoveKeyName, AliasNames2[Selection] ) );
				if( AliasNames3[Selection] != "" )
					GetPlayerOwner().ConsoleCommand("SET Input "@RemoveKeyName@RemoveActionFromKey( RemoveKeyName, AliasNames3[Selection] ) );

				BoundKey2[Selection] = 0;
			}
		}
		else 
		if( KeyNo == BoundKey2[Selection] )
		{			
			// *** Remove key in slot1
			RemoveKeyName = RealKeyName[BoundKey1[Selection]];
			GetPlayerOwner().ConsoleCommand("SET Input "@RemoveKeyName@RemoveActionFromKey( RemoveKeyName, AliasNames1[Selection] ) );
			if( AliasNames2[Selection] != "" )
				GetPlayerOwner().ConsoleCommand("SET Input "@RemoveKeyName@RemoveActionFromKey( RemoveKeyName, AliasNames2[Selection] ) );
			if( AliasNames3[Selection] != "" )
				GetPlayerOwner().ConsoleCommand("SET Input "@RemoveKeyName@RemoveActionFromKey( RemoveKeyName, AliasNames3[Selection] ) );

			BoundKey1[Selection] = BoundKey2[Selection];
			BoundKey2[Selection] = 0;
		}
		else
		{
			// Clear out old slot 2 if it exists
			if( BoundKey2[Selection] != 0 )
			{
				// *** Remove key in slot2
				RemoveKeyName = RealKeyName[BoundKey2[Selection]];
				GetPlayerOwner().ConsoleCommand("SET Input "@RemoveKeyName@RemoveActionFromKey( RemoveKeyName, AliasNames1[Selection] ) );
				if( AliasNames2[Selection] != "" )
					GetPlayerOwner().ConsoleCommand("SET Input "@RemoveKeyName@RemoveActionFromKey( RemoveKeyName, AliasNames2[Selection] ) );
				if( AliasNames3[Selection] != "" )
					GetPlayerOwner().ConsoleCommand("SET Input "@RemoveKeyName@RemoveActionFromKey( RemoveKeyName, AliasNames3[Selection] ) );

				BoundKey2[Selection] = 0;
			}
			
			// move key 1 to key 2, and set ourselves in 1.
			BoundKey2[Selection] = BoundKey1[Selection];
			BoundKey1[Selection] = KeyNo;
			
			// From here down is where we will add an action to a key
			if( AliasNames2[Selection] == "" )
			{
				GetPlayerOwner().ConsoleCommand("SET Input"@KeyName@AddActionToKey( KeyName, AliasNames1[Selection]) );
			}
			else
			{
				if( AliasNames3[Selection] == "" )
				{	
					GetPlayerOwner().ConsoleCommand("SET Input"@KeyName@AddActionToKey( KeyName, AliasNames1[Selection]) );
					GetPlayerOwner().ConsoleCommand("SET Input"@KeyName@AddActionToKey( KeyName, AliasNames2[Selection]) );	
				}
				else
				{
					GetPlayerOwner().ConsoleCommand("SET Input"@KeyName@AddActionToKey( KeyName, AliasNames1[Selection]) );
					GetPlayerOwner().ConsoleCommand("SET Input"@KeyName@AddActionToKey( KeyName, AliasNames2[Selection]) );	
					GetPlayerOwner().ConsoleCommand("SET Input"@KeyName@AddActionToKey( KeyName, AliasNames3[Selection]) );	
				}
			}
		}
	}
	else
	{
		BoundKey1[Selection] = KeyNo;

		if( AliasNames2[Selection] == "" )
		{
//			GetPlayerOwner().ConsoleCommand("SET Input"@KeyName@AliasNames1[Selection]);
			GetPlayerOwner().ConsoleCommand("SET Input"@KeyName@AddActionToKey( KeyName, AliasNames1[Selection]) );
//			log( "called consoleCommand " $"SET Input"@KeyName@NewAlias );
		}
		else
		{
			if( AliasNames3[Selection] == "" )
			{
				GetPlayerOwner().ConsoleCommand("SET Input"@KeyName@AddActionToKey( KeyName, AliasNames1[Selection]) );
				GetPlayerOwner().ConsoleCommand("SET Input"@KeyName@AddActionToKey( KeyName, AliasNames2[Selection]) );	
			}
			else
			{
				GetPlayerOwner().ConsoleCommand("SET Input"@KeyName@AddActionToKey( KeyName, AliasNames1[Selection]) );
				GetPlayerOwner().ConsoleCommand("SET Input"@KeyName@AddActionToKey( KeyName, AliasNames2[Selection]) );	
				GetPlayerOwner().ConsoleCommand("SET Input"@KeyName@AddActionToKey( KeyName, AliasNames3[Selection]) );	
			}
		}
	}
}

function ProcessKey( int KeyNo )
{
	local string keyName;
	keyName = RealKeyName[KeyNo];

	log("OptionPage '"$AliasNames1[selection] $"' attempt to set as " $keyNo $":"$KeyName);
	PlayClick();

	if ( (KeyName == "") || (KeyName == "Escape")  
		|| ((KeyNo >= 0x70 ) && (KeyNo <= 0x79)) // function keys
		|| ((KeyNo >= 0x30 ) && (KeyNo <= 0x39))// number keys
		|| (KeyNo ==91) || (KeyNo ==92) || (KeyNo == 93) // windows keys
		|| (KeyNo==236) || (keyNo==237)
		) 
		return;
	
	RemoveExistingKey(KeyNo, KeyName);
	
	SetKey( KeyNo, KeyName );

}

function bool KeyEvent( byte/*EInputKey*/ KeyNo, byte/*EInputAction*/ Action, FLOAT Delta )
{
	if (Action == 1 
	&& bPolling
	&& keyNo >= 5 // Handle mouse clicks separately

	) // button press
	{
		ProcessKey(KeyNo);

		bPolling = False;
		SelectedButton.bDisabled = False;
		return true;
	}

	return super.KeyEvent(KeyNo, Action, Delta);
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
		
		case DifficultyCombo:
			DifficultyChanged();
			break;
		
		case SensitivitySlider:
			SensitivityChanged();
			break;
			
		case MouseSmoothCheck:
			MouseSmoothChanged();
			break;
			
		case InvertBroomCheck:
			InvertBroomChanged();
			break;

		case AutoJumpCheck:
			autoJumpChanged();
			break;

		case MouseInvertCheck:
			MouseInvertChanged();
			break;
		
		case AutoCenterCamCheck:
			AutoCenterCamChanged();
			break;
		
		case AutoDrinkPotionCheck:
			AutoDrinkPotionChanged();
			break;

		case MoveWhileCastingCheck:
			MoveWhileCastingChanged();
			break;
		}
		
		case DE_Click:
			
			if( bPolling )
			{
				bPolling = False;
				SelectedButton.bDisabled = False;
				
				if(C == SelectedButton)
				{
					ProcessKey(1);
					return;
				}
			}
			
			if (UWindowButton(C) != None)
			{
				PlayClick();
				for ( I=0; I<ArrayCount(InGame_KeyButtons); I++ )
				{
					if (InGame_KeyButtons[I] == C)
					{
						// Remove only the in-game keys when making this key assignment
						RemoveExistingBoundKeyMinIndex = 0;
						RemoveExistingBoundKeyMaxIndex = InGame_LastBoundKeyIndex;
						SetKeyType = KT_Game;

						SelectedButton = UWindowButton(C);
						Selection = I;
						log("OptionPage '"$AliasNames1[Selection] $"' clicked ...");
						
						bPolling = True;
						log ("Polling set to true");
						SelectedButton.bDisabled = True;
						return;
					}
				}
				for ( I=0; I<ArrayCount(InQuidditch_KeyButtons); I++ )
				{
					if (InQuidditch_KeyButtons[I] == C)
					{
						// Remove no keys when making this key assignment
						RemoveExistingBoundKeyMinIndex = InQuidditch_FirstBoundKeyIndex;
						RemoveExistingBoundKeyMaxIndex = InQuidditch_LastBoundKeyIndex;
						SetKeyType = KT_Quidditch;

						SelectedButton = UWindowButton(C);
						Selection = InQuidditch_FirstBoundKeyIndex+I;
						log("OptionPage '"$AliasNames1[Selection] $"' clicked ...");
						
						bPolling = True;
						log ("Polling set to true");
						SelectedButton.bDisabled = True;
						return;
					}
				}
				for ( I=0; I<1/*ArrayCount(InWizardDuel_KeyButtons)*/; I++ )
				{
					if (InWizardDuel_KeyButtons[I] == C)
					{
						// Remove no keys when making this key assignment
						RemoveExistingBoundKeyMinIndex = InWizardDuel_FirstBoundKeyIndex;
						RemoveExistingBoundKeyMaxIndex = InWizardDuel_LastBoundKeyIndex;
						SetKeyType = KT_WizardDuel;

						SelectedButton = UWindowButton(C);
						Selection = InWizardDuel_FirstBoundKeyIndex+I;
						log("OptionPage '"$AliasNames1[Selection] $"' clicked ...");
						
						bPolling = True;
						log ("Polling set to true");
						SelectedButton.bDisabled = True;
						return;
					}
				}
			}
			
			
			switch( C )
			{
				case BackPageButton:
					FEBook(book).DoEscapeFromPage();
					return;
			}

			break;
			
		case DE_RClick:
			if (bPolling)
			{
				bPolling = False;
				SelectedButton.bDisabled = False;
				
				if(C == SelectedButton)
				{
					ProcessKey(2);
					return;
				}
			}
			break;
			
		case DE_MClick:
			if (bPolling)
			{
				bPolling = False;
				SelectedButton.bDisabled = False;
				
				if(C == SelectedButton)
				{
					ProcessKey(4);
					return;
				}			
			}
			break;
	}
}


defaultproperties
{
	InGame_Text="In Game"
	InWizardDuel_Text="In Wizard Duel"
	InQuidditch_Text="In Quidditch"

//	MouseHiText="High"
//	MouseLoText="Low"
	SensitivityText="Sensitivity"
	MouseSmoothText="Mouse Smoothing"
	

	// -------------------------------------------------
	// --- In Game Keys
	// InGame
	InGame_LabelList(0)="Forward"
	InGame_LabelList(1)="Backward"
	InGame_LabelList(2)="Turn Left"
	InGame_LabelList(3)="Turn Right"
	InGame_LabelList(4)="Jump"
	InGame_LabelList(5)="Use Wand"
	InGame_LabelList(6)="Drink Potion"
	InGame_LabelList(7)="Strafe Left"
	InGame_LabelList(8)="Strafe Right"

	// InQuidditch
  // --- In Quidditch Keys
	InQuidditch_LabelList(0)="Speed Up"
	InQuidditch_LabelList(1)="Speed Down"
	
	// InWizardDuel
	InWizardDuel_LabelList(0)="Cycle Spell"
	
	
	AliasNames1(0)="MoveForward"
	AliasNames1(1)="MoveBackward"
	AliasNames1(2)="RotLeft"
	AliasNames1(3)="RotRight"
	AliasNames1(4)="Jump"
	AliasNames1(5)="AltFire"
	AliasNames1(6)="button bDrinkWiggenwell"
	AliasNames1(7)="StrafeLeft"
	AliasNames1(8)="StrafeRight"
	AliasNames1(9)="button bBroomBoost"
	AliasNames1(10)="button bBroomBrake"
	AliasNames1(11)="button bDuelCycleSpell"
	
	AliasNames2(0)="button bBroomPitchUp"
	AliasNames2(1)="button bBroomPitchDown"
	AliasNames2(2)="button bBroomYawLeft"
	AliasNames2(3)="button bBroomYawRight"
	
	AliasNames3(0)="button bSpellLessonUp"
	AliasNames3(1)="button bSpellLessonDown"
	AliasNames3(2)="button bSpellLessonLeft"
	AliasNames3(3)="button bSpellLessonRight"
	AliasNames3(4)=""
	AliasNames3(5)="button bVendorReply"


	// -------------------------------------------------
	// InGame
	InGame_VertSpacing(0)=35
	InGame_VertSpacing(1)=35
	InGame_VertSpacing(2)=35
	InGame_VertSpacing(3)=35
	InGame_VertSpacing(4)=35
	InGame_VertSpacing(5)=35
	InGame_VertSpacing(6)=35
	InGame_VertSpacing(7)=35
	InGame_VertSpacing(8)=35

	// InQuidditch
	InQuidditch_VertSpacing(0)=30
	InQuidditch_VertSpacing(1)=30
	
	
	// InWizardDuel
	InWizardDuel_VertSpacing(0)=35


	KeyboardText="Keyboard"

	LocalizedKeyName(1)="LeftMouse"
	LocalizedKeyName(2)="RightMouse"
	LocalizedKeyName(3)="Cancel"
	LocalizedKeyName(4)="MiddleMouse"
	LocalizedKeyName(5)="Unknown05"
	LocalizedKeyName(6)="Unknown06"
	LocalizedKeyName(7)="Unknown07"
	LocalizedKeyName(8)="Backspace"
	LocalizedKeyName(9)="Tab"
	LocalizedKeyName(10)="Unknown0A"
	LocalizedKeyName(11)="Unknown0B"
	LocalizedKeyName(12)="Unknown0C"
	LocalizedKeyName(13)="Enter"
	LocalizedKeyName(14)="Unknown0E"
	LocalizedKeyName(15)="Unknown0F"
	LocalizedKeyName(16)="Shift"
	LocalizedKeyName(17)="Ctrl"
	LocalizedKeyName(18)="Alt"
	LocalizedKeyName(19)="Pause"
	LocalizedKeyName(20)="CapsLock"
	LocalizedKeyName(21)="Unknown15"
	LocalizedKeyName(22)="Unknown16"
	LocalizedKeyName(23)="Unknown17"
	LocalizedKeyName(24)="Unknown18"
	LocalizedKeyName(25)="Unknown19"
	LocalizedKeyName(26)="Unknown1A"
	LocalizedKeyName(27)="Escape"
	LocalizedKeyName(28)="Unknown1C"
	LocalizedKeyName(29)="Unknown1D"
	LocalizedKeyName(30)="Unknown1E"
	LocalizedKeyName(31)="Unknown1F"
	LocalizedKeyName(32)="Space"
	LocalizedKeyName(33)="PageUp"
	LocalizedKeyName(34)="PageDown"
	LocalizedKeyName(35)="End"
	LocalizedKeyName(36)="Home"
	LocalizedKeyName(37)="Left"
	LocalizedKeyName(38)="Up"
	LocalizedKeyName(39)="Right"
	LocalizedKeyName(40)="Down"
	LocalizedKeyName(41)="Select"
	LocalizedKeyName(42)="Print"
	LocalizedKeyName(43)="Execute"
	LocalizedKeyName(44)="PrintScrn"
	LocalizedKeyName(45)="Insert"
	LocalizedKeyName(46)="Delete"
	LocalizedKeyName(47)="Help"
	LocalizedKeyName(48)="0"
	LocalizedKeyName(49)="1"
	LocalizedKeyName(50)="2"
	LocalizedKeyName(51)="3"
	LocalizedKeyName(52)="4"
	LocalizedKeyName(53)="5"
	LocalizedKeyName(54)="6"
	LocalizedKeyName(55)="7"
	LocalizedKeyName(56)="8"
	LocalizedKeyName(57)="9"
	LocalizedKeyName(58)="Unknown3A"
	LocalizedKeyName(59)="Unknown3B"
	LocalizedKeyName(60)="Unknown3C"
	LocalizedKeyName(61)="Unknown3D"
	LocalizedKeyName(62)="Unknown3E"
	LocalizedKeyName(63)="Unknown3F"
	LocalizedKeyName(64)="Unknown40"
	LocalizedKeyName(65)="A"
	LocalizedKeyName(66)="B"
	LocalizedKeyName(67)="C"
	LocalizedKeyName(68)="D"
	LocalizedKeyName(69)="E"
	LocalizedKeyName(70)="F"
	LocalizedKeyName(71)="G"
	LocalizedKeyName(72)="H"
	LocalizedKeyName(73)="I"
	LocalizedKeyName(74)="J"
	LocalizedKeyName(75)="K"
	LocalizedKeyName(76)="L"
	LocalizedKeyName(77)="M"
	LocalizedKeyName(78)="N"
	LocalizedKeyName(79)="O"
	LocalizedKeyName(80)="P"
	LocalizedKeyName(81)="Q"
	LocalizedKeyName(82)="R"
	LocalizedKeyName(83)="S"
	LocalizedKeyName(84)="T"
	LocalizedKeyName(85)="U"
	LocalizedKeyName(86)="V"
	LocalizedKeyName(87)="W"
	LocalizedKeyName(88)="X"
	LocalizedKeyName(89)="Y"
	LocalizedKeyName(90)="Z"
	LocalizedKeyName(91)="Unknown5B"
	LocalizedKeyName(92)="Unknown5C"
	LocalizedKeyName(93)="Unknown5D"
	LocalizedKeyName(94)="Unknown5E"
	LocalizedKeyName(95)="Unknown5F"
	LocalizedKeyName(96)="NumPad0"
	LocalizedKeyName(97)="NumPad1"
	LocalizedKeyName(98)="NumPad2"
	LocalizedKeyName(99)="NumPad3"
	LocalizedKeyName(100)="NumPad4"
	LocalizedKeyName(101)="NumPad5"
	LocalizedKeyName(102)="NumPad6"
	LocalizedKeyName(103)="NumPad7"
	LocalizedKeyName(104)="NumPad8"
	LocalizedKeyName(105)="NumPad9"
	LocalizedKeyName(106)="GreyStar"
	LocalizedKeyName(107)="GreyPlus"
	LocalizedKeyName(108)="Separator"
	LocalizedKeyName(109)="GreyMinus"
	LocalizedKeyName(110)="NumPadPeriod"
	LocalizedKeyName(111)="GreySlash"
	LocalizedKeyName(112)="F1"
	LocalizedKeyName(113)="F2"
	LocalizedKeyName(114)="F3"
	LocalizedKeyName(115)="F4"
	LocalizedKeyName(116)="F5"
	LocalizedKeyName(117)="F6"
	LocalizedKeyName(118)="F7"
	LocalizedKeyName(119)="F8"
	LocalizedKeyName(120)="F9"
	LocalizedKeyName(121)="F10"
	LocalizedKeyName(122)="F11"
	LocalizedKeyName(123)="F12"
	LocalizedKeyName(124)="F13"
	LocalizedKeyName(125)="F14"
	LocalizedKeyName(126)="F15"
	LocalizedKeyName(127)="F16"
	LocalizedKeyName(128)="F17"
	LocalizedKeyName(129)="F18"
	LocalizedKeyName(130)="F19"
	LocalizedKeyName(131)="F20"
	LocalizedKeyName(132)="F21"
	LocalizedKeyName(133)="F22"
	LocalizedKeyName(134)="F23"
	LocalizedKeyName(135)="F24"
	LocalizedKeyName(136)="Unknown88"
	LocalizedKeyName(137)="Unknown89"
	LocalizedKeyName(138)="Unknown8A"
	LocalizedKeyName(139)="Unknown8B"
	LocalizedKeyName(140)="Unknown8C"
	LocalizedKeyName(141)="Unknown8D"
	LocalizedKeyName(142)="Unknown8E"
	LocalizedKeyName(143)="Unknown8F"
	LocalizedKeyName(144)="NumLock"
	LocalizedKeyName(145)="ScrollLock"
	LocalizedKeyName(146)="Unknown92"
	LocalizedKeyName(147)="Unknown93"
	LocalizedKeyName(148)="Unknown94"
	LocalizedKeyName(149)="Unknown95"
	LocalizedKeyName(150)="Unknown96"
	LocalizedKeyName(151)="Unknown97"
	LocalizedKeyName(152)="Unknown98"
	LocalizedKeyName(153)="Unknown99"
	LocalizedKeyName(154)="Unknown9A"
	LocalizedKeyName(155)="Unknown9B"
	LocalizedKeyName(156)="Unknown9C"
	LocalizedKeyName(157)="Unknown9D"
	LocalizedKeyName(158)="Unknown9E"
	LocalizedKeyName(159)="Unknown9F"
	LocalizedKeyName(160)="LShift"
	LocalizedKeyName(161)="RShift"
	LocalizedKeyName(162)="LControl"
	LocalizedKeyName(163)="RControl"
	LocalizedKeyName(164)="UnknownA4"
	LocalizedKeyName(165)="UnknownA5"
	LocalizedKeyName(166)="UnknownA6"
	LocalizedKeyName(167)="UnknownA7"
	LocalizedKeyName(168)="UnknownA8"
	LocalizedKeyName(169)="UnknownA9"
	LocalizedKeyName(170)="UnknownAA"
	LocalizedKeyName(171)="UnknownAB"
	LocalizedKeyName(172)="UnknownAC"
	LocalizedKeyName(173)="UnknownAD"
	LocalizedKeyName(174)="UnknownAE"
	LocalizedKeyName(175)="UnknownAF"
	LocalizedKeyName(176)="UnknownB0"
	LocalizedKeyName(177)="UnknownB1"
	LocalizedKeyName(178)="UnknownB2"
	LocalizedKeyName(179)="UnknownB3"
	LocalizedKeyName(180)="UnknownB4"
	LocalizedKeyName(181)="UnknownB5"
	LocalizedKeyName(182)="UnknownB6"
	LocalizedKeyName(183)="UnknownB7"
	LocalizedKeyName(184)="UnknownB8"
	LocalizedKeyName(185)="UnknownB9"
	LocalizedKeyName(186)="Semicolon"
	LocalizedKeyName(187)="Equals"
	LocalizedKeyName(188)="Comma"
	LocalizedKeyName(189)="Minus"
	LocalizedKeyName(190)="Period"
	LocalizedKeyName(191)="Slash"
	LocalizedKeyName(192)="Tilde"
	LocalizedKeyName(193)="UnknownC1"
	LocalizedKeyName(194)="UnknownC2"
	LocalizedKeyName(195)="UnknownC3"
	LocalizedKeyName(196)="UnknownC4"
	LocalizedKeyName(197)="UnknownC5"
	LocalizedKeyName(198)="UnknownC6"
	LocalizedKeyName(199)="UnknownC7"
	LocalizedKeyName(200)="Joy1"
	LocalizedKeyName(201)="Joy2"
	LocalizedKeyName(202)="Joy3"
	LocalizedKeyName(203)="Joy4"
	LocalizedKeyName(204)="Joy5"
	LocalizedKeyName(205)="Joy6"
	LocalizedKeyName(206)="Joy7"
	LocalizedKeyName(207)="Joy8"
	LocalizedKeyName(208)="Joy9"
	LocalizedKeyName(209)="Joy10"
	LocalizedKeyName(210)="Joy11"
	LocalizedKeyName(211)="Joy12"
	LocalizedKeyName(212)="Joy13"
	LocalizedKeyName(213)="Joy14"
	LocalizedKeyName(214)="Joy15"
	LocalizedKeyName(215)="Joy16"
	LocalizedKeyName(216)="UnknownD8"
	LocalizedKeyName(217)="UnknownD9"
	LocalizedKeyName(218)="UnknownDA"
	LocalizedKeyName(219)="LeftBracket"
	LocalizedKeyName(220)="Backslash"
	LocalizedKeyName(221)="RightBracket"
	LocalizedKeyName(222)="SingleQuote"
	LocalizedKeyName(223)="UnknownDF"
	LocalizedKeyName(224)="JoyX"
	LocalizedKeyName(225)="JoyY"
	LocalizedKeyName(226)="JoyZ"
	LocalizedKeyName(227)="JoyR"
	LocalizedKeyName(228)="MouseX"
	LocalizedKeyName(229)="MouseY"
	LocalizedKeyName(230)="MouseZ"
	LocalizedKeyName(231)="MouseW"
	LocalizedKeyName(232)="JoyU"
	LocalizedKeyName(233)="JoyV"
	LocalizedKeyName(234)="UnknownEA"
	LocalizedKeyName(235)="UnknownEB"
	LocalizedKeyName(236)="MouseWheelUp"
	LocalizedKeyName(237)="MouseWheelDown"
	LocalizedKeyName(238)="Unknown10E"
	LocalizedKeyName(239)="Unknown10F"
	LocalizedKeyName(240)="JoyPovUp"
	LocalizedKeyName(241)="JoyPovDown"
	LocalizedKeyName(242)="JoyPovLeft"
	LocalizedKeyName(243)="JoyPovRight"
	LocalizedKeyName(244)="UnknownF4"
	LocalizedKeyName(245)="UnknownF5"
	LocalizedKeyName(246)="Attn"
	LocalizedKeyName(247)="CrSel"
	LocalizedKeyName(248)="ExSel"
	LocalizedKeyName(249)="ErEof"
	LocalizedKeyName(250)="Play"
	LocalizedKeyName(251)="Zoom"
	LocalizedKeyName(252)="NoName"
	LocalizedKeyName(253)="PA1"
	LocalizedKeyName(254)="OEMClear"
	

	OrString="or"
	InvertBroomText="Invert Broom"
	AutoJumpText="Auto Jump"
	optionsText="OPTIONS"

	DifficultyText= "Difficulty"
	DifficultyLevel(0)="Easy"
	DifficultyLevel(1)="Medium"
	DifficultyLevel(2)="Hard"

	LabelTextColor=(R=40,G=180,B=40)
	GoupLabelTextColor=(R=255,G=255,B=255)
	ButtonTextColor=(R=255,G=255,B=255)
}
