//=============================================================================
// HPConsole - console replacer to implement UWindow UI System
//=============================================================================
class  HPConsole extends baseConsole;


#exec TEXTURE IMPORT NAME=MainBack1 FILE=Textures\Menu\MainBack1.bmp GROUP="Icons" MIPS=OFF
#exec TEXTURE IMPORT NAME=MainBack2 FILE=Textures\Menu\MainBack2.bmp GROUP="Icons" MIPS=OFF
#exec TEXTURE IMPORT NAME=MainBack3 FILE=Textures\Menu\MainBack3.bmp GROUP="Icons" MIPS=OFF
#exec TEXTURE IMPORT NAME=MainBack4 FILE=Textures\Menu\MainBack4.bmp GROUP="Icons" MIPS=OFF
#exec TEXTURE IMPORT NAME=MainBack5 FILE=Textures\Menu\MainBack5.bmp GROUP="Icons" MIPS=OFF
#exec TEXTURE IMPORT NAME=MainBack6 FILE=Textures\Menu\MainBack6.bmp GROUP="Icons" MIPS=OFF

#exec TEXTURE IMPORT NAME=MainBackUp FILE=Textures\Menu\MainUp.bmp GROUP="Icons" MIPS=OFF
#exec TEXTURE IMPORT NAME=MainBackDown FILE=Textures\Menu\MainDown.bmp GROUP="Icons" MIPS=OFF
#exec TEXTURE IMPORT NAME=MainBackOver FILE=Textures\Menu\MainOver.bmp GROUP="Icons" MIPS=OFF

#exec TEXTURE IMPORT NAME=DemoUp FILE=Textures\Menu\DemoUp.bmp GROUP="Icons" MIPS=OFF
#exec TEXTURE IMPORT NAME=DemoDown FILE=Textures\Menu\DemoDown.bmp GROUP="Icons" MIPS=OFF
#exec TEXTURE IMPORT NAME=DemoOver FILE=Textures\Menu\DemoOver.bmp GROUP="Icons" MIPS=OFF


#exec TEXTURE IMPORT NAME=LevelUp FILE=Textures\Menu\LevelUp.bmp GROUP="Icons" MIPS=OFF
#exec TEXTURE IMPORT NAME=LevelDown FILE=Textures\Menu\LevelDown.bmp GROUP="Icons" MIPS=OFF
#exec TEXTURE IMPORT NAME=LevelOver FILE=Textures\Menu\LevelOver.bmp GROUP="Icons" MIPS=OFF

var UWindowDialogClientWindow levSelect;

//var UWindowBitmap letterWin;

var UWindowButton OKButton;
const BUTTONS_X	=185;
const BUTTONS_Y =193;
var bool bLevelSelect;
var bool firstRun;
var int CanvasSizeX,CanvasSizeY;
var float fFadeDirection;

var bool bShootingRange;

var bool bLoadNewLevel;
var bool bFastForwardMode;
//var bool bShowMenu;

//var hudStoryBook storyBook;

var FEBook menuBook;

var bool bToggleMoveModePressed;
var bool bShowPos;

//var bool bBossCamera;
var bool bBoostKeyPressed;

//var bool bArrowKeyLeftPressed;
//var bool bArrowKeyRightPressed;
//var bool bArrowKeyUpPressed;
//var bool bArrowKeyDownPressed;


var ParticleFX MouseParticle;

var bool bShiftDown;

var UWindowMessageBoxCW TestConfirm;

var bool	bInHubFlow;		// Whether level is being played like within the normal hub-to-hub flow
							// of the game (as opposed to direct play outside of normal hub flow)
							// Note: this is true even if level was launched from the Level Select
							// menu because that's meant to simulate in-hub flow during testing).

var string ResTimeOutSettings;


var ShortCutWindow	SCWindow;

var CutLogWindow CutConsoleWindow;
var bool bShowCutConsole;

var float fSlomoSpeed;

var bool bVendorBar;    // True if vendor hud is up (console will update and display the 
                        // the mouse if it is)

//debugging command to destroy all actors of a certain class.
//I used it to determine how much run time classes were taking.
exec function DestroyClass(string input)
{
	Harry(Viewport.Actor).DestroyClass(input);
}
exec function ListGroups()
{
	Harry(Viewport.Actor).ListGroups();
}


exec function ShortCut()
{
	if( SCWindow == None )
	{
		// This is our first time calling the ShortCut function so create our window
//		HPConsole(root.console).Viewport.Actor.ClientMessage("Created SCWindow -> " $SCWindow );
		SCWindow = ShortCutWindow( Root.CreateWindow( class'ShortCutWindow', 64, 64, 320, 320) );
	}
	else if( true == SCWindow.bUWindowActive )
	{
		// Window is already active, hiding window
//		HPConsole(root.console).Viewport.Actor.ClientMessage("Window is already visible, hiding window" );
		SCWindow.Close();
	}
	else
	{
		// Window is not active, showing window
//		HPConsole(root.console).Viewport.Actor.ClientMessage("Window is not visible, showing window" );
		SCWindow.ActivateWindow(0, false);
	}
}

function CutConsoleLog(string msg)
{
	CutLogClientWindow(CutConsoleWindow.ClientArea).TextArea.AddText(msg);
}
function ShowCutConsole(bool flag)
{

	if(flag)
		{
		bShowCutConsole=true;
		CutConsoleWindow.ShowWindow();
		}
	else
		{
		bShowCutConsole=false;
		CutConsoleWindow.HideWindow();
		}

}

// Lumos Debug functions
exec function Lumos_Debug()				 
{ 
	if( baseWand(Harry(Viewport.Actor).weapon).TheLumosLight != None )
		baseWand(Harry(Viewport.Actor).weapon).TheLumosLight.ShowDebugInfo();
}


// Wand
exec function Wand_Debug			( bool bInput  ) { baseWand(Harry(Viewport.Actor).weapon).SetDebugMode( bInput );	}

// SpellCursor
exec function SpellCursor_Debug		( bool bInput  ) { Harry(Viewport.Actor).SpellCursor.SetDebugMode( bInput );		}
exec function SpellCursor_Distance	( float	fInput ) { Harry(Viewport.Actor).SpellCursor.SetLOSDistance( fInput );		}

// Camera Functions
exec function Cam_Mode			( string sInput) { Harry(Viewport.Actor).cam.SetModeByString( sInput );		}
exec function Cam_Settings		(		       ) { Harry(Viewport.Actor).cam.ShowSettings();				}
exec function Cam_SaveSettings	( int   iInput ) { Harry(Viewport.Actor).cam.SaveUserSettings( iInput );	}
exec function Cam_LoadSettings	( int   iInput ) { Harry(Viewport.Actor).cam.LoadUserSettings( iInput );	}
exec function Cam_MinPitch		( float fInput ) { Harry(Viewport.Actor).cam.SetMinPitch( fInput );			}
exec function Cam_MaxPitch		( float fInput ) { Harry(Viewport.Actor).cam.SetMaxPitch( fInput );			}
exec function Cam_XOffset		( float fInput ) { Harry(Viewport.Actor).cam.SetXOffset( fInput );			}
exec function Cam_YOffset		( float fInput ) { Harry(Viewport.Actor).cam.SetYOffset( fInput );			}
exec function Cam_ZOffset		( float fInput ) { Harry(Viewport.Actor).cam.SetZOffset( fInput );			}
exec function Cam_Distance		( float fInput ) { Harry(Viewport.Actor).cam.SetDistance( fInput );			}
exec function Cam_RotStepYaw	( float fInput ) { Harry(Viewport.Actor).cam.SetRotStepYaw( fInput );		}
exec function Cam_RotStepPitch	( float fInput ) { Harry(Viewport.Actor).cam.SetRotStepPitch( fInput );		}
exec function Cam_RotStepRoll	( float fInput ) { Harry(Viewport.Actor).cam.SetRotStepRoll( fInput );		}
exec function Cam_RotTightness	( float fInput ) { Harry(Viewport.Actor).cam.SetRotTightness( fInput ); 	}
exec function Cam_RotSpeed		( float fInput ) { Harry(Viewport.Actor).cam.SetRotSpeed( fInput ); 		}
exec function Cam_MoveTightness	( float fInput ) { Harry(Viewport.Actor).cam.SetMoveTightness( fInput );	}
exec function Cam_MoveSpeed		( float fInput ) { Harry(Viewport.Actor).cam.SetMoveSpeed( fInput ); 		}
exec function Cam_Yaw			( float fInput ) { Harry(Viewport.Actor).cam.SetYaw( fInput ); 				}
exec function Cam_Pitch			( float fInput ) { Harry(Viewport.Actor).cam.SetPitch( fInput ); 			}
exec function Cam_Roll			( float fInput ) { Harry(Viewport.Actor).cam.SetRoll( fInput ); 			}
exec function Cam_Target		( name  nInput ) { Harry(Viewport.Actor).cam.SetTargetActor( nInput );		}
exec function Cam_SyncPos		( bool  bInput ) { Harry(Viewport.Actor).cam.SetSyncPosWithTarget( bInput );}
exec function Cam_SyncRot		( bool  bInput ) { Harry(Viewport.Actor).cam.SetSyncRotWithTarget( bInput );}
exec function Cam_FOV			( float fInput ) { Harry(Viewport.Actor).cam.SetFOV( fInput );				}
exec function Cam_CutCommand	( string sInput) { Harry(Viewport.Actor).cam.CutCommand( sInput );			}

// Boss Tweak functions
exec function Boss              ( string sInput) { baseBoss(Harry(Viewport.Actor).BossTarget).TweakSetting( sInput );		}

// GameState functions
exec function SetGState			( string str   ) { Harry(Viewport.Actor).SetGameState( str );				 }
exec function ShowGState		()				 { Harry(Viewport.Actor).ClientMessage("Current GameState : "$Harry(Viewport.Actor).CurrentGameState );}

// Wizard card functions
exec function ShowCardData      ()               { Harry(Viewport.Actor).managerStatus.ShowCardData();		 }

// Housepoint functions
exec function AddHPointsG       (int nPoints)    { Harry(Viewport.Actor).managerStatus.AddHPointsG(nPoints); }
exec function AddHPointsH       (int nPoints)    { Harry(Viewport.Actor).managerStatus.AddHPointsH(nPoints); }
exec function AddHPointsS       (int nPoints)    { Harry(Viewport.Actor).managerStatus.AddHPointsS(nPoints); }
exec function AddHPointsR       (int nPoints)    { Harry(Viewport.Actor).managerStatus.AddHPointsR(nPoints); }

// Potion ingredients
exec function AddFMucus         (int nCount)     { Harry(Viewport.Actor).managerStatus.AddFMucus(nCount);    }
exec function AddWBark          (int nCount)     { Harry(Viewport.Actor).managerStatus.AddWBark(nCount);     }

// Polyjuice ingredients
exec function AddBicorn         (int nCount)     { Harry(Viewport.Actor).managerStatus.AddBicorn(nCount);    }
exec function AddBoomslang      (int nCount)     { Harry(Viewport.Actor).managerStatus.AddBoomslang(nCount);     }

// Potions
exec function AddPotions        (int nCount)     { Harry(Viewport.Actor).managerStatus.AddPotions(nCount);   }

// Jellybeans
exec function AddBeans          (int nCount)     { Harry(Viewport.Actor).managerStatus.AddBeans(nCount);     }

// Health
exec function AddHealth         (int nCount)     { Harry(Viewport.Actor).managerStatus.AddHealth(nCount);    }
exec function AddHealthPotential(int nCount)     { Harry(Viewport.Actor).managerStatus.AddHealthPotential(nCount);}

// Wizard cards
exec function GiveCardToHarry       (int nCardId){ Harry(Viewport.Actor).managerStatus.GiveCardToHarry(nCardId);   }
exec function GiveAllCardsToHarry   ()			 { Harry(Viewport.Actor).managerStatus.GiveAllCardsToHarry();	   }
exec function GiveCardToVendors     (int nCardId){ Harry(Viewport.Actor).managerStatus.GiveCardToVendors(nCardId); }
//exec function GiveAllCardsToVendors ()		 { Harry(Viewport.Actor).managerStatus.GiveAllCardsToVendors();    }

// SpellBook
exec function GiveSpell			( string str )	 { Harry(Viewport.Actor).AddToSpellBookByString( str );		 }
exec function GiveAllSpells		( )				 { Harry(Viewport.Actor).AddAllSpellsToSpellBook();			 }
exec function TakeAllSpells		( )				 { Harry(Viewport.Actor).ClearSpellBook();					 }

// Countdown or BeanRoom timer
exec function ShowTimer(bool bShow)
{
    local CountdownTimerManager TimerManager;

	foreach Viewport.Actor.AllActors(class'CountdownTimerManager', TimerManager)
		TimerManager.bShowNumericTime = bShow;
}

exec function DuelingMode( bool bOn )
{
/*	local  Harry	playerHarry;
	local  Duellist duelOpponent;
	
	ForEach AllActors(class'Harry', playerHarry)
		break;
	
	ForEach AllActors(class'Duellist', duelOpponent)
		break;
*/
	if( bOn )
	{
		Harry(Viewport.Actor).TurnOnDuelingMode( None );
//		duelOpponent.gotostate('stateStartDuel');
	}
	else
	{
		Harry(Viewport.Actor).TurnOffDuelingMode();
//		duelOpponent.gotoState('stateIdle');
//		duelOpponent.DuellistAnimChannel.gotoState('stateIdle');
	}
}

function ToggleDebugMode()
{
	if(!class'Version'.default.bDebugEnabled)
		{
//		bDebugMode=false;
//		SaveConfig();
		return;
		}

	bDebugMode=!bDebugMode;
//	SaveConfig();
}


function SaveSelectedSlot()
{
	StopFastforward();		//just in case.
	MenuBook.SaveSelectedSlot();
}
function LoadSelectedSlot()
{
	MenuBook.LoadSelectedSlot();
	StopFastforward();		//just in case.
}


function ShowConsole()
{
	if(!bDebugMode)
		return;
		
	bShowConsole = true;
	if(bCreatedRoot)
		ConsoleWindow.ShowWindow();

}
function HideConsole()
{
	bShowConsole = false;
	if(bCreatedRoot)
		ConsoleWindow.HideWindow();

}


// Called in order to initiate a level change.
function ChangeLevel(string lev,bool flag)
{
	Log("Changing level to:"$lev $"," $flag);
	viewport.Actor.Level.ServerTravel( lev, flag );
	bLoadNewLevel = true;
}


function LaunchUWindow(optional bool bPause)
{
	super.LaunchUWindow(bPause);
}

// Override to change bNoDrawWorld.
function CloseUWindow()
{
	super.CloseUWindow();
}

event Tick(float delta)
{
/*	if( Viewport.Actor.Level.NextURL == "" )
	{
		if( bLoadNewLevel )
		{
			// Save game as soon as level is loaded. 
			// This is true when NextURL is no longer set.
			// First tick of new level.
			bLoadNewLevel = false;
			Log("Saving level at start");
			SaveSelectedSlot();
			MenuBook.OnLevelLoadDone();
		}
	}
*/


	//kludge to insure menu comes up first.
	if(firstRun==false)
		{
		LaunchUWindow();//start menus
//		CloseUWindow();
		firstRun=true;
		}

	if(bFastForwardMode)
		{
		if(!baseHud(harry(viewport.actor).myHud).bCutSceneMode)
			StopFastForward();
		if(!bSpacePressed)
			StopFastForward();
		}
	else
		{
		if(bSpacePressed && baseHud(harry(viewport.actor).myHud).bCutSceneMode)
			{
			StartFastForward();
			}
		}


		//tick any windows
	if(Root != None)
		Root.DoTick(Delta);	
}
function StartFastforward()
{
	if(!bDebugMode)
		return;

	if(harry(viewport.actor)==None)
		return;

	if(baseHud(harry(viewport.actor).myHud)==None)
		return;

	if(!hpHud(harry(viewport.actor).myHud).bCutSceneMode)
		return;
	
	Harry(viewport.actor).SloMo(8.0);
	bFastForwardMode=true;
}
function StopFastforward()
{
	Harry(viewport.actor).NormalSpeed();
	bFastForwardMode=false;
}


function doLevelSave (int i)
{
	local string savePauser;
	local PlayerPawn playerPawn;
	local GameSaveInfo gameSaveInfo;
	local int n;

	// AWRIGHT_111001_001
	local int SavePointID;


	StopFastforward();		//just in case.

	playerPawn = viewport.Actor;

		// disable and store pauser if active

	savePauser = playerPawn.Level.Pauser;
	playerPawn.Level.Pauser = "";

		// Save the actual game

	//playerPawn.Level.ConsoleCommand("SaveGame " $i);
    playerPawn.SaveGame(i);

	playerPawn.Level.Pauser = savePauser;

		// Save game info

	gameSaveInfo = new class'GameSaveInfo';
/*
	gameSaveInfo.numBeans  = Harry(playerPawn).numBeans;
	gameSaveInfo.numStars  = Harry(playerPawn).numStars;
	gameSaveInfo.numPoints = Harry(playerPawn).getNumHousePointsHarry ();
*/
	n = InStr(playerPawn.level.LevelEnterText, ".");
	if (n==-1)
		gameSaveInfo.currentLevelString = playerPawn.level.LevelEnterText;
	else
		gameSaveInfo.currentLevelString = Left(playerPawn.level.LevelEnterText, n);
		
	log("LevelSave: Level Name is" $gameSaveInfo.currentLevelString);


	// AWRIGHT_111001_001
	// find near spell book & store instance number 
	// in game save info for later displaying correct 
	// thumbnail in the game slot selection screen

	SavePointID = -1;

	SavePointID = Harry(viewport.actor).FindNearestSavePointID();

	Log( "Found Savepoint ID = " $SavePointID );

	// save save point instance 
	// number in save game info

	gameSaveInfo.savePointID = SavePointID;

	// AWRIGHT_111001_001 - end




//	playerPawn.SaveGameSaveInfo(nSelectedSlot$"GameSaveInfo"$i, gameSaveInfo);

	playerPawn.SaveGameSaveInfo("GameSaveInfo"$i, gameSaveInfo);


		// Save screenShot

	// Not working right.	
	// AWRIGHT_091001_002
	// root.console.viewport.Actor.ConsoleCommand("Snap 3");
	// root.console.viewport.Actor.ConsoleCommand("SaveSnap ..\\Save\\SaveGameSnap" $i $".bmp");
	//root.console.viewport.Actor.ConsoleCommand( 
	//	"SaveSnap128 ..\\Save\\SaveGameSnap" $i $".bmp");
}

exec function LangBrowser()
{
	menuBook.OpenBook("Lang");
}



function DrawE3DemoLockout(Canvas canvas)
{
local float w,h;
local font saveFont;

	saveFont=canvas.font;
	canvas.font=root.fonts[0];
	Canvas.SetPos(10,460);
	Canvas.DrawText("Locked");
	canvas.font=saveFont;
}


state UWindow
{

	event bool KeyType( EInputKey Key )
	{
		if (Root != None)
			Root.WindowEvent(WM_KeyType, None, MouseX, MouseY, Key);
		return True;
	}


	event bool KeyEvent( EInputKey Key, EInputAction Action, FLOAT Delta )
	{
		local byte k;

		//log("HPConsole keyEvent");
		if(ResTimeOutSettings != "")
		{
			log("HPConsole : setRes");
			viewport.actor.ConsoleCommand("SetRes "$ResTimeOutSettings);
			ResTimeOutSettings = "";

			//if (FEOptionsPage(menuBook.curPage) != None)
			//	FEOptionsPage(menuBook.curPage).LoadAvailableSettings();
		}

		k = Key;

		if( menuBook.KeyEvent( Key, Action, Delta ) )
			{
			return true;
			}
		else // not processed
			{
			switch( Action )
			{
				case IST_Release:
					switch(k)
					{
						case EInputKey.IK_LEFTMOUSE:
							if(Root != None) 
								Root.WindowEvent(WM_LMouseUp, None, MouseX, MouseY, k);
							break;
					}
				break;
				
				case IST_PRESS:
					switch(k)
					{
						case EInputKey.IK_F4:
							SCWindow.Close();
							CloseUWindow();
							break;
						
						case EInputKey.IK_F7:
							ToggleDebugMode();
							break;
					}
				break;
			}

/*			if(Action==IST_Release && Key==IK_LeftMouse)
				{
				if(Root != None) 
					Root.WindowEvent(WM_LMouseUp, None, MouseX, MouseY, k);
				}

			if(action==IST_PRESS && key==IK_F7)	// test
				ToggleDebugMode();
*/
			return Super.KeyEvent(Key, Action, Delta);
			}

	}

Begin:
}


exec function ShowPos()
{
	bShowPos = !bShowPos;
}


exec function giveAllCards ()
{
	GiveAllCardsToHarry();
}


function ExitFromGame()
{
	MenuBook.ExitFromGame();
}

event bool KeyEvent( EInputKey Key, EInputAction Action, FLOAT Delta )
{
	local byte k;

	k = Key;

	switch(Action)
	{
	case IST_Release:
		switch(k)
			{
			case EInputKey.IK_LEFTMOUSE:
				bspaceReleased = true;
			//	return True;
				break;

			case EInputKey.IK_SPACE:
				//This is also used for
				bSpacePressed = false;
				bBoostKeyPressed = false;

				Harry(viewport.actor).bSkipKeyPressed = false;
				break;
			
			case EInputKey.IK_Pause:
			case EInputKey.IK_Cancel:
				Harry(viewport.actor).ConsoleCommand( "pause" );
				break;

			case EInputKey.IK_PageUp:	
				if( fSlomoSpeed >= 1.0f )
				{
					if( fSlomoSpeed < 20 )
						fSlomoSpeed += 0.5f;
				}
				else
					fSlomoSpeed = 1.0f;
				
				Harry(viewport.actor).Slomo( fSlomoSpeed );
				Harry(viewport.actor).ClientMessage(" ^^^ Setting GameSpeed to: X" $fSlomoSpeed );
				break;
			
			case EInputKey.IK_PageDown:
				if( fSlomoSpeed <= 1.0f )
					fSlomoSpeed *= 0.5f;
				else
					fSlomoSpeed = 1.0f;
				
				Harry(viewport.actor).SloMo( fSlomoSpeed );
				Harry(viewport.actor).ClientMessage(" ^^^ Setting GameSpeed to: X" $fSlomoSpeed );
				break;
			
			case EInputKey.IK_NumPad4:
				bLeftKeyDown = false;
				break;

			case EInputKey.IK_NumPad6:
				bRightKeyDown = false;
				break;

			case EInputKey.IK_NumPad8:
				bForwardKeyDown = false;
				break;

			case EInputKey.IK_NumPad2:
				bbackKeyDown = false;
				break;

			case EInputKey.IK_NumPad1:
				bRotateLeftKeyDown = false;
				break;

			case EInputKey.IK_NumPad3:
				bRotateRightKeyDown = false;
				break;

			case EInputKey.IK_NumPad0:
				bRotateUpKeyDown = false;
				break;

			case EInputKey.IK_NumPadPeriod:
				bRotateDownKeyDown = false;
				break;

			case EInputKey.IK_NumPad7:
				bUpKeyDown = false;
				break;

			case EInputKey.IK_NumPad9:
				bDownKeyDown = false;

			case EInputKey.IK_Shift:
				bShiftDown = False;
				break;

			}
		break;


	case IST_Axis:
        if (bVendorBar)
        {
		    switch (Key)
		    {
		    case IK_MouseX:
			    MouseX = MouseX + (MouseScale * Delta);
			    break;
		    case IK_MouseY:
			    MouseY = MouseY - (MouseScale * Delta);
			    break;					
		    }
        }

        break;

	case IST_Press:

		if(	Harry(viewport.actor) != none )
			Harry(viewport.actor).KeyDownEvent( int(Key) );

		switch(k)
			{
			//case EInputKey.IK_Left:
			//case EInputKey.IK_Up:
			//case EInputKey.IK_Right:
			//case EInputKey.IK_Down:


			case EInputKey.IK_Shift:
				bShiftDown = True;
				break;

			case EInputKey.IK_TAB:
				if(bDebugMode)
					Type();
				break;
			
			case EInputKey.IK_NumPad4:
				bLeftKeyDown = true;
				break;

			case EInputKey.IK_NumPad6:
				bRightKeyDown = true;
				break;

			case EInputKey.IK_NumPad8:
				bForwardKeyDown = true;
				break;

			case EInputKey.IK_NumPad2:
				bbackKeyDown = true;
				break;

			case EInputKey.IK_NumPad1:
				bRotateLeftKeyDown = true;
				break;

			case EInputKey.IK_NumPad3:
				bRotateRightKeyDown = true;
				break;

			case EInputKey.IK_NumPad0:
				bRotateUpKeyDown = true;
				break;

			case EInputKey.IK_NumPadPeriod:
				bRotateDownKeyDown = true;
				break;

			case EInputKey.IK_NumPad7:
				bUpKeyDown = true;
				break;

			case EInputKey.IK_NumPad9:
				bDownKeyDown = true;
				break;

		// Save screenShot

			case EInputKey.IK_Insert:
//				if(bDebugMode)
					viewport.Actor.sshot();
				break;

			case EInputKey.IK_RIGHTMOUSE:
				break;
			case EInputKey.IK_Escape:
				// AMM
				// MenuBook.OpenBook("REPORT");
				MenuBook.EscFromConsole();
				return true;

			case EInputKey.IK_Equals:
				MenuBook.DoMapFromConsole();
				return true;

			case ConsoleKey:
				if (bLocked)
					return true;

				Root.bAllowConsole=class'Version'.default.bDebugEnabled;
				if(!bDebugMode)
					return true;
				
				if(bShiftDown)
					{
					ShowCutConsole(!bShowCutConsole);
					return true;
					}	

				bQuickKeyEnable = True;
				LaunchUWindow();
				if(!bShowConsole)
					ShowConsole();
				return true;
			case EinputKey.IK_LEFTMOUSE:
				bspaceReleased=false;
			//	return(true);
				break;

			case EInputKey.IK_SPACE:
				bSpacePressed = true;
				bBoostKeyPressed = true;
				Harry(viewport.actor).bSkipKeyPressed = true;
				//Harry(viewport.actor).clientmessage("Skip True");
				break;

			case EInputKey.IK_F4:	// test
				if(bDebugMode)
				{
					// Put our console into UWindow mode so that we have a mouse
					LaunchUWindow();
					
					// Turn on our shortcut menu
					ShortCut();

					// Hide the menuBook (we don't want to see the level buttons)
					//menuBook.HideWindow();
				}
					//baseHarry(viewport.actor).GotoShortcut(2);
				break;
			case EInputKey.IK_F6:	// test
				Harry(viewport.actor).GetHealthStatusItem().SetCountToMaxPotential();
				break;

			case EInputKey.IK_F7:	// test
				ToggleDebugMode();
				break;
			case EInputKey.IK_F8:	// test
				break;
			case EInputKey.IK_F9:	// test
				
				//*** DEBUG ***
				//*TEST* we will get rid of this later.. but for now lets add all spells on F8
				Harry(viewport.actor).AddAllSpellsToSpellBook();
				//*************
				break;
				// Switch strafing mode
				//baseHarry(viewport.actor).cam.bUseStrafing = !baseHarry(viewport.actor).cam.bUseStrafing;
				//if (baseHarry(viewport.actor).cam.IsInState('BossState'))
				//{
				//	// reset state
				//	if (baseHarry(viewport.actor).cam.bUseStrafing)
				//	{
				//		baseHarry(viewport.actor).MovementMode(true);
				//	}
				//	else
				//	{
				//		baseHarry(viewport.actor).MovementMode(false);
				//	}
				//}
				//break;

			case eInputKey.IK_F10:
			//	doLevelSave(99);
				break;

			case eInputKey.IK_F11:
				MenuBook.ExitFromConsole();
				return true;

			case EInputKey.IK_F12:	
				break;
			}
		break;
	}
		
	

	return False; 
	//!! because of ConsoleKey
	//!! return Super.KeyEvent(Key, Action, Delta);
}

function handleMenuEvent()
{

}

function drawLegal(Canvas canvas)
{
}

function newDrawBack( canvas Canvas )
{
}
function drawBack( canvas Canvas )
{
}

function SetupLanguage()
{
local string f1,f2,f3,f4;
local int f1s,f2s,f3s,f4s;


	LanguageCode=GetLanguage();

	log("LanguageCode="$LanguageCode);


	switch(caps(LanguageCode))
		{
		case "SIM":
		case "CHI":
		case "TRA":
		case "KOR":
		case "THA":
		case "JAP":
			bUseAsianFont=true;
			f1=Localize("all","Font1Name", "SAPFont");
			f1s=int(Localize("all","Font1Size", "SAPFont"));
			f2=Localize("all","Font2Name", "SAPFont");
			f2s=int(Localize("all","Font2Size", "SAPFont"));
			f3=Localize("all","Font3Name", "SAPFont");
			f3s=int(Localize("all","Font3Size", "SAPFont"));
			f4=Localize("all","Font4Name", "SAPFont");
			f4s=int(Localize("all","Font4Size", "SAPFont"));

			log("Font1:" $f1 $":" $f1s);
			log("Font2:" $f2 $":" $f2s);
			log("Font3:" $f3 $":" $f3s);
			log("Font4:" $f4 $":" $f4s);

			LocalBigFont=CreateNativeFont(f1,f1s);
			LocalMedFont=CreateNativeFont(f2,f2s);
			LocalSmallFont=CreateNativeFont(f3,f3s);
			LocalTinyFont=CreateNativeFont(f4,f4s);
			LocalIconMessageFont=LocalBigFont;

			root.Fonts[0]=LocalSmallFont;
			root.Fonts[1]=LocalSmallFont;
			root.Fonts[2]=LocalMedFont;
			root.Fonts[3]=LocalMedFont;
			root.Fonts[4]=LocalMedFont;

			break;

/*		case "THA":
			bUseThaiFont=true;
			LocalBigFont=Font'ThaiFontBig';
			LocalMedFont=Font'ThaiFontMed';
			LocalSmallFont=Font'ThaiFontSmall';
			LocalTinyFont=Font'ThaiFontSmall';
			LocalIconMessageFont=LocalBigFont;

			root.Fonts[0]=Font'ThaiFontSmall';
			root.Fonts[1]=Font'ThaiFontSmall';
			root.Fonts[2]=Font'ThaiFontMed';
			root.Fonts[3]=Font'ThaiFontMed';
			root.Fonts[4]=Font'ThaiFontMed';
			break;
		case "JAP":
			bUseAsianFont=true;
			LocalBigFont=CreateNativeFont(JapFont1Name, JapFont1Size);
			LocalMedFont=CreateNativeFont(JapFont2Name, JapFont2Size);
			LocalSmallFont=CreateNativeFont(JapFont3Name, JapFont3Size);
			LocalTinyFont=CreateNativeFont(JapFont4Name, JapFont4Size);
			LocalIconMessageFont=LocalBigFont;

			root.Fonts[0]=LocalSmallFont;
			root.Fonts[1]=LocalSmallFont;
			root.Fonts[2]=LocalMedFont;
			root.Fonts[3]=LocalMedFont;
			root.Fonts[4]=LocalMedFont;
			break;
*/

		case "POL":		//
			LocalBigFont=Font(DynamicLoadObject("HPFonts.PolFontLarge", class'Font'));
			LocalMedFont=Font(DynamicLoadObject("HPFonts.PolFontMed", class'Font'));
			LocalSmallFont=Font(DynamicLoadObject("HPFonts.PolFontSmall", class'Font'));
			LocalTinyFont=Font(DynamicLoadObject("HPFonts.PolFontTiny", class'Font'));

			root.Fonts[0]=LocalTinyFont;
			root.Fonts[1]=LocalSmallFont;
			root.Fonts[2]=LocalSmallFont;
			root.Fonts[3]=LocalSmallFont;
			root.Fonts[4]=LocalSmallFont;

			LocalIconMessageFont=LocalMedFont;
			break;
		case "ENG":		//
		case "INT":		//
			LocalBigFont=Font'HugeInkFont';
			LocalMedFont=Font'BigInkFont';
			LocalSmallFont=Font'MedInkFont';
			LocalTinyFont=Font'SmallInkFont';
			LocalIconMessageFont=LocalSmallFont;
			break;
		case "GER":
		default:
			LocalBigFont=Font'BigInkFont';
			LocalMedFont=Font'MedInkFont';
			LocalSmallFont=Font'SmallInkFont';
			LocalTinyFont=Font'TinyInkFont';
			LocalIconMessageFont=Font'SmallInkFont';
			break;
		}

/*
bUseSystemFonts=false;
	if(bUseAsianFont)
		{
if(false)//		if(bUseSystemFonts)
			{
			LocalBigFont=Font'SystemFontBig';
			LocalMedFont=Font'SystemFontMed';
			LocalSmallFont=Font'SystemFontSmall';
			LocalTinyFont=Font'SystemFontSmall';
			LocalIconMessageFont=LocalBigFont;

			root.Fonts[0]=Font'SystemFontSmall';
			root.Fonts[1]=Font'SystemFontSmall';
			root.Fonts[2]=Font'SystemFontMed';
			root.Fonts[3]=Font'SystemFontMed';
			root.Fonts[4]=Font'SystemFontMed';
			}
		else
			{
			}
		}
	if(bUseThaiFont)
		{
		}
*/

	SaveConfig();

}
event PostRender( canvas Canvas )
{
local LevelInfo lev;

	Super.PostRender(Canvas);

//log(self $"########################In PostRender");

/*	if(!bCreatedRoot) 
		{

log("########################Creating root window");

		CreateRootWindow(Canvas);
		root.SetScale(root.RealWidth/640);

		SetupLanguage();

		menuBook=FEBook(Root.CreateWindow(class'FEBook', 0*((Root.WinWidth/2)-320), 0*((Root.WinHeight/2)-240), 640, 480, root));

			//special case if this level was launched from the editor.
		lev=Root.GetLevel();
		log("Init level = " $lev.GetLocalUrl());
		if( InStr(caps(lev.GetLocalUrl()),"STARTUP")<0 )
			{	//yup so bypass the menus
log("########################Here1");
			MenuBook.bGamePlaying=true;
			MenuBook.CloseBook();
			}
		else
			{
log("########################Here2");
			MenuBook.bGamePlaying=false;
			MenuBook.OpenBook("Main");
			}

		}

*/
	if (Root != None)
		{
		if(MenuBook.bIsOpen || bShowCutConsole || bVendorBar)
			RenderUWindow( Canvas );
		}

	if( bShowPos )
		{
		Canvas.DrawColor.R = 255;
		Canvas.DrawColor.G = 255;
		Canvas.DrawColor.B = 255;
		Canvas.SetPos( Canvas.SizeX-200, Canvas.SizeY-40 );
		Canvas.DrawText( "Player @ "$ 
			int(Viewport.Actor.Location.X) $","$
			int(Viewport.Actor.Location.Y) $","$
			int(Viewport.Actor.Location.Z) );
		}

	if(Harry(viewport.actor).bE3DemoLockout)
		DrawE3DemoLockout(canvas);
}

function RenderUWindow( canvas Canvas )
{
local LevelInfo lev;

	local UWindowWindow NewFocusWindow;

	local Texture curTexture;

	Canvas.bNoSmooth = True;
	Canvas.Z = 1;
	Canvas.Style = 1;
	Canvas.DrawColor.r = 255;
	Canvas.DrawColor.g = 255;
	Canvas.DrawColor.b = 255;


	if(Viewport.bWindowsMouseAvailable && Root != None)
	{
		MouseX = Viewport.WindowsMouseX/Root.GUIScale;
		MouseY = Viewport.WindowsMouseY/Root.GUIScale;
	}


	if(!bCreatedRoot) 
		{
		CreateRootWindow(Canvas);
		root.SetScale(root.RealWidth/640);

		SetupLanguage();

		menuBook=FEBook(Root.CreateWindow(class'FEBook', 0*((Root.WinWidth/2)-320), 0*((Root.WinHeight/2)-240), 640, 480, root));

			//special case if this level was launched from the editor.
/*		lev=Root.GetLevel();
		log("Init level = " $lev.GetLocalUrl());
		if( InStr(caps(lev.GetLocalUrl()),"STARTUP")<0 )
			{	//yup so bypass the menus
			MenuBook.bGamePlaying=true;
			MenuBook.CloseBook();
			}
		else
			{
			MenuBook.bGamePlaying=false;
			MenuBook.OpenBook("Main");
			}
*/
		MenuBook.bGamePlaying=true;
		MenuBook.CloseBook();
		HideConsole();
		}

	Root.bWindowVisible = True;
	Root.bUWindowActive = bUWindowActive;
	Root.bQuickKeyEnable = bQuickKeyEnable;

	if(Canvas.ClipX != OldClipX || Canvas.ClipY != OldClipY)
	{
		OldClipX = Canvas.ClipX;
		OldClipY = Canvas.ClipY;
		
		Root.WinTop = 0;
		Root.WinLeft = 0;
		Root.WinWidth = Canvas.ClipX / Root.GUIScale;
		Root.WinHeight = Canvas.ClipY / Root.GUIScale;

		Root.RealWidth = Canvas.ClipX;
		Root.RealHeight = Canvas.ClipY;

		Root.ClippingRegion.X = 0;
		Root.ClippingRegion.Y = 0;
		Root.ClippingRegion.W = Root.WinWidth;
		Root.ClippingRegion.H = Root.WinHeight;

		Root.Resized();
	}

	if(MouseX > Root.WinWidth) MouseX = Root.WinWidth;
	if(MouseY > Root.WinHeight) MouseY = Root.WinHeight;
	if(MouseX < 0) MouseX = 0;
	if(MouseY < 0) MouseY = 0;


	// Check for keyboard focus
	NewFocusWindow = Root.CheckKeyFocusWindow();

	if(NewFocusWindow != Root.KeyFocusWindow)
	{
		Root.KeyFocusWindow.KeyFocusExit();		
		Root.KeyFocusWindow = NewFocusWindow;
		Root.KeyFocusWindow.KeyFocusEnter();
	}


	Root.MoveMouse(MouseX, MouseY);
	Root.WindowEvent(WM_Paint, Canvas, MouseX, MouseY, 0);
	if(bUWindowActive || bQuickKeyEnable) 
		{
		Root.DrawMouse(Canvas);
		}

}

event DrawLevelInfo( canvas C, string URL )
{
	local float sizeX, sizeY;
	local string index;
	local string text;
	local int dot, cards, secrets;

	sizeX = 256.0*FrameX/640.0;
	sizeY = 256.0*FrameY/480.0;

	/*
	C.CurX = 0;  C.CurY = 0;		C.DrawRect(Texture'HPLevelInfoBackground1', sizeX, sizeY);
	C.CurX = sizeX;					C.DrawRect(Texture'HPLevelInfoBackground2', sizeX, sizeY);
	C.CurX = 2*sizeX;				C.DrawRect(Texture'HPLevelInfoBackground3', sizeX, sizeY);
	C.CurX = 0;  C.CurY = sizeY;	C.DrawRect(Texture'HPLevelInfoBackground4', sizeX, sizeY);
	C.CurX = sizeX;					C.DrawRect(Texture'HPLevelInfoBackground5', sizeX, sizeY);
	C.CurX = 2*sizeX;				C.DrawRect(Texture'HPLevelInfoBackground6', sizeX, sizeY);
	*/
	// Convert file name to level title and objective.
	dot = InStr( URL, "." );
	if( dot >= 0 )
		URL = Left( URL, dot );
	log("NextURL = "$ URL);

	C.bCenter = true;

	// Clip to edges of parchment.
	C.OrgX = FrameX * 0.15;		
	C.ClipX = FrameX * 0.7;

	C.DrawColor.R = 0;  C.DrawColor.G = 0;  C.DrawColor.B = 0;

	// Convert level name to index.
	index = Localize( "text", "n_"$ URL, "Dobby" );
	
	// Level name.
	text = Localize( "text", "level_name_"$ index, "HGame" );
	if( Left(text,1) != "<" )
	{
		C.CurX = FrameX * 0.35;
		C.CurY = FrameY * 0.25;
		C.Font = LocalBigFont;
		C.DrawText( text );
	}

	// Objective.
	text = Localize( "text", "objective_"$ index, "HGame" );
	if( Left(text,1) != "<" )
	{
		C.CurX = FrameX * 0.35;
		C.CurY = FrameY * 0.5;
		C.Font = LocalMedFont;
		C.DrawText( text ); 
	}

	// Only show secret counts if in regular game flow.
	if( bInHubFlow )
	{
		text = Localize( "text", "secret_"$ URL, "Dobby" );
		cards = int(Left(text, 1));
		secrets = int(Mid(text, 2, 1));

		C.Font = LocalSmallFont;
		C.CurY = FrameY * 0.7;
		if( cards > 0 )
		{
			C.CurX = FrameX * 0.35;
			text = Localize( "all", "find_wizard_text_0"$ cards, "Pickup" );
			if( Left(text,1) != "<" )
				C.DrawText( text );
			C.CurY = FrameY * 0.75;
		}
		if( secrets > 0 )
		{
			C.CurX = FrameX * 0.35;
			text = Localize( "all", "find_secret_text_0"$ secrets, "Pickup" );
			if( Left(text,1) != "<" )
				C.DrawText( text );
		}
	}
}

defaultproperties
{
	fSlomoSpeed=1.0f

    bShootingRange=True
    FadeoutTime=0.5
    FadeinTime=1
    PausedMessage="PRESS ESC TO EXIT"
    PrecachingMessage="ENTERING"

}
