//=============================================================================
// ShortCutBrowser - A Short cut browser class, used to make development easier
//=============================================================================
class ShortCutBrowser extends UWindowDialogClientWindow;


struct List
{
	var	string			strName;		// name of list
	var ShortCutList	list;			// list of shortcuts
};

var UWindowComboControl	ComboBoxListTypes;	// combo box that displays the current list
var UWindowComboControl	ComboBoxGameStates;		// combo box that displays the current GameState

var ShortCutButton		buttonLaunch;			// launch button to launch the current shortcut
//var ShortCutButton		buttonCancel;			// cancel button to deActivate the shortcut window



var int					iNumLists;		// total number of lists (each list has a name and a list)
var int					iCurList;		// currently selected list index
var List				ListArray[8];	// array used to display the list of shortcuts
const 					MAX_LISTS	= 8;



var int					iNumStates;		// total number of states
var int					iCurState;		// currently selected state index
var array<string>		StateArray;		// array used to display the gamestates


function Created()
{
	Super.Created();
	
	bTransient = false;

	// Create List ComboBox 
	ComboBoxListTypes = UWindowComboControl( CreateControl(class'UWindowComboControl',0,0,128,16));
	ComboBoxListTypes.SetFont( F_Normal );
	ComboBoxListTypes.SetEditable(false);
	ComboBoxListTypes.SetButtons(false);
	ComboBoxListTypes.SetHelpText("ComboBoxListTypes help text");
	
	
	// Create GameState ComboBox
	ComboBoxGameStates = UWindowComboControl( CreateControl(class'UWindowComboControl',128,0,64,16));
	ComboBoxGameStates.SetFont( F_Normal );
	ComboBoxGameStates.SetEditable(false);
	ComboBoxGameStates.SetButtons(false);
	ComboBoxGameStates.SetHelpText("ComboBoxGameStates help text");

	// Create Launch Button
	buttonLaunch = ShortCutButton( CreateControl( class'ShortCutButton', 192, 0, 64, 15 ) );
	buttonLaunch.ShowWindow();
	buttonLaunch.CancelAcceptsFocus();
	buttonLaunch.ToolTipString	= "Launch current shortcut";
	buttonLaunch.SetText("Launch");
//	buttonLaunch.Align=TA_Center

	// Create Cancel Button
/*	buttonCancel = ShortCutButton( CreateControl(class'ShortCutButton', 252, 0, 64, 15));
	buttonCancel.ShowWindow();
	buttonCancel.CancelAcceptsFocus();
	buttonCancel.ToolTipString	= "Close shortcut window";
	buttonCancel.SetText("Cancel");
//	buttonCancel.Align=TA_Center
*/	
	// store the name of our current list
	iCurList	= 0;
}

function Activated()
{
	UpdateCurrentGameStateSelection();
}

function Resized()
{
	// add resize support with our list	
	listArray[iCurList].list.WinWidth  = WinWidth;
	listArray[iCurList].list.WinHeight = WinHeight-4; // -4 so that our list lines up with our frame size
	listArray[iCurList].list.Resized();
	
	Super.Resized();
}

function AddList( string strName, ShortCutList newList )
{
	// paramater checks
	if( newList == None || iNumLists+1 >= MAX_LISTS)
		return;
	
	// add this new list name to our combo box
	ComboBoxListTypes.AddItem( strName );
	
	// store a refrence to the new list
	listArray[iNumLists].list    = newList;
	listArray[iNumLists].list.HideWindow();//Deactivated();
	
	listArray[iNumLists].strName = strName;
	
	if( iNumLists == 0 )
	{
		// If this is our first list set this as our current list
		ComboBoxListTypes.SetSelectedIndex(0);
		ListTypeComboBoxChanged();
	}
	
	// inc our number of lists
	iNumLists++;
}

function UpdateCurrentGameStateSelection()
{
	ComboBoxGameStates.SetSelectedIndex( 
		ComboBoxGameStates.List.FindItemIndex( GetPlayerOwner().CurrentGameState, true) );
}

function AddGameState( string strState )
{
	// paramater checks
	if( Len(strState) == 0 )
		return;
	
	// add this new list name to our combo box
	ComboBoxGameStates.AddItem( strState );
	
/*
	if( GetPlayerOwner().CurrentGameState ~= strState )
	{
		// If this is our first list set this as our current list
		ComboBoxGameStates.SetSelectedIndex( ComboBoxGameStates.FindItemIndex(strState, true) );
	}
*/

	// inc our number of lists
	iNumStates++;
}

function Notify(UWindowDialogControl C, byte E)
{
	Super.Notify(C, E);

	switch(E)
	{
		case DE_Change:
			switch(C)
			{
				case ComboBoxListTypes:		
					ListTypeComboBoxChanged();	
					break;
				
				case ComboBoxGameStates:
					GameStateComboBoxChanged();	
					break;	
			}
			break;
		
		case DE_Click:
			switch(C)
			{
				case buttonLaunch:

					listArray[iCurList].list.OnLaunchButton();

					FocusWindow();
					break;

//				case buttonCancel:	
//					Super.Close();
//					break;
			}
			break;
	}
}

function GameStateComboBoxChanged()
{
	// get harry then call his SetGameState function
	Harry(GetPlayerOwner()).SetGameState( ComboBoxGameStates.GetValue() );
}

function ListTypeComboBoxChanged()
{
	local string strNewName;
	local int	 i;

	// Find out the index of our selected list
	// We need to extract the index from strNewName
	
	i = 0;
	strNewName = ComboBoxListTypes.GetValue();

//	*** Debug
//	HPConsole(root.console).Viewport.Actor.ClientMessage( 
//	 "ComboBoxListTypes changed to name -> " $strNewName );

	while(i < MAX_LISTS)
	{
		if( strNewName == listArray[i].strName )
		{
			// We found the new list so lets load the new list into our browser
			
			// hide our current list
			listArray[iCurList].list.HideWindow();
			listArray[iCurList].list.CancelAcceptsFocus();
			
			// show our new current list
			iCurList   = i;
			listArray[i].list.ShowWindow();
			listArray[i].list.SetAcceptsFocus();
			listArray[i].list.Reset();

			// Make sure our selected list matches the size of our browser
			Resized();

//			*** Debug
//			HPConsole(root.console).Viewport.Actor.ClientMessage( 
//			"found -> " $ComboBoxListTypes.GetValue() $" index -> " $i );
			return;
		}
		++i;
	}
}



/*
function Paint(Canvas canvas,float x,float y)
{

}

event bool KeyEvent( EInputKey Key, EInputAction Action, FLOAT Delta )
{
	switch(Action)
	{
		case IST_Press:

		if(	baseHarry(viewport.actor) != none )
			baseHarry(viewport.actor).KeyDownEvent( int(Key) );

		switch(k)
		{
			case EInputKey.IK_TAB:
		}
	}

}
*/

// ****************************
// **** Default Properties ****
// ****************************
