//===============================================================================
//  CountdownTimerManager
//  
//  A countdown timer can be started in a number of ways:
//
//     1) If bStartOnLevelLoad is true, the timer graphic will come up right
//        away on level load and begin counting down.
//     2) If the CountdownTimerManager gets a trigger event, that event acts as 
//        a toggle.  If the timer hasn't been started, it will start.  If the
//        timer is currently going, it will be stopped and the graphic will 
//        go away.
//     3) The CountdownTimerManager can be captured in a cutscence script
//        (cutname = "CountdownTimerManager") and started from the script via
//        a StartCountdown command.
//
//  No matter how the timer was started, it can be stopped or end in any of the
//  following ways.  In all cases, the timer graphic will go away.
//
//     1) The timer reaches the end.
//     2) If the timer is currently going, a trigger can toggle it to stop.
//     3) A cutscene script can call the StopCountdown command.
//
//  If the timer ends because the time is up, the event setup in the 
//  CountdownTimerManager's Events\Event property will be sent out so that 
//  other objects can trigger off of the timer completion.  No event will be sent
//  out if the count is manually stopped with a trigger or cutscene script 
//  command.
//
//===============================================================================

class CountdownTimerManager extends HudItemManager;

#EXEC TEXTURE IMPORT NAME=TimerBarFull  FILE=TEXTURES\Menu\HUD\Timer.bmp GROUP="Icons" FLAGS=2 MIPS=OFF
#EXEC TEXTURE IMPORT NAME=TimerBarEmpty  FILE=TEXTURES\Menu\HUD\EmptyBar.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

// Exposed properties
var(CountdownTimerManager) bool    bStartOnLevelLoad; // start the clock on level load?
var(CountdownTimerManager) float   fDuration;         // Duration of countdown

// Internal
var float fCountdownTime;                             // Curr countdown time
var float fLastTickTime;                              // Countdown remaning at last tick
var bool  bShowNumericTime;                           // True to show numbers for debugging

//-----------------------------------------------------------------------------------
//  Functions
//-----------------------------------------------------------------------------------


event PostBeginPlay()
{
	// If want to start the countdown when level loads, we set a timer
	// and will start from that timer.  We hold off on starting the countdown
	// until the timer goes off because we need access to Harry in order to get
	// access to the hud and start the countdown.  Harry is the last thing 
	// created a level load, so we will not be able to get to him here.
	if (bStartOnLevelLoad == true)
		SetTimer(0.2, false);
}

event Timer()
{
	if (bStartOnLevelLoad)
	{
		// Harry is the last thing in the level to load and we need him in order
		// to access the hud and register ourselves with it.  Until we can get 
		// to Harry, keep resetting the timer.
		if (Level.PlayerHarryActor == None)
			SetTimer(0.2, false);

		// Harry has been created for the level!  We can now start the countdown.
		else
		{
			// Start the countdown.
			if (bStartOnLevelLoad)
				StartCountDown();
		}
	}
}

// Called from CutScene script.
function bool CutCommand(string command, optional string cue, optional bool bFastFlag)
{
	local string  sActualCommand;
	local string  sCutName;
	local actor   a;
	
	sActualCommand = ParseDelimitedString( command, " ", 1, false );


	if( sActualCommand ~= "Capture" )
	{
		return (true);
	}

	else 
	if( sActualCommand ~= "Release" )
	{
		return (true);
	}

	else
	if( sActualCommand ~= "StartCountdown" )
	{
		StartCountdown();
		CutNotifyActor.CutCue( cue );
		return (true);
	}
	else
	if( sActualCommand ~= "StopCountdown" )
	{
		StopCountdown();
		CutNotifyActor.CutCue( cue );
		return (true);
	}
	else
		return (false);
}

function float GetTimerDuration()
{
	return (fDuration);
}

function StartCountDown()
{
	HPHud((Level.PlayerHarryActor).myHud).RegisterCountdownTimerManager(self);
	GoToState('CountingDown');
}

function StopCountDown()
{
	HPHud((Level.PlayerHarryActor).myHud).RegisterCountdownTimerManager(None);
	GoToState('Idle');
}

function DrawCountdown(Canvas canvas)
{
	local int		Ox, Oy;
	local texture	TimerFull;
	local float		TimeRemaining;

	// Draw the timer	
	TimerFull = Texture'TimerBarFull';

	Ox = Canvas.SizeX - 8 - TimerFull.USize;
	Oy = Canvas.SizeY - 160;

	Canvas.SetPos(Ox, Oy);
	Canvas.DrawIcon(Texture'TimerBarEmpty',1);

	TimeRemaining = fCountdownTime / GetTimerDuration();

	Canvas.SetPos(ox, oy);
	Canvas.DrawTile(TimerFull, (TimerFull.USize - 97 ) * TimeRemaining + 97, TimerFull.VSize, 0, 0, (TimerFull.USize - 97) * TimeRemaining + 97, TimerFull.VSize);

	DrawTuningModeData(Canvas);
}

	
function DrawTuningModeData(Canvas canvas)
{
	local string strCurrTime;

	if (bShowNumericTime)
	{
		Canvas.SetPos(Canvas.SizeX - 75, Canvas.SizeY - 60);
		strCurrTime = string(int(GetTimerDuration() - fCountdownTime));
		Canvas.DrawText(strCurrTime, false);
	}
}

function PlayCountdownSound()
{
	if (abs(fLastTickTime - fCountdownTime) > 1.0)
	{
		// Play timer tick
		if (fCountdownTime > 30.0)
			PlaySound(Sound'HPSounds.menu_sfx.timer_1', SLOT_None, 0.5);
		else if (fCountdownTime > 20.0)
			PlaySound(Sound'HPSounds.menu_sfx.timer_2', SLOT_None, 0.5);
		else if (fCountdownTime > 15.0)
			PlaySound(Sound'HPSounds.menu_sfx.timer_3', SLOT_None, 0.5);
		else if (fCountdownTime > 10.0)
			PlaySound(Sound'HPSounds.menu_sfx.timer_4', SLOT_None, 0.5);
		else if (fCountdownTime > 5.0)
			PlaySound(Sound'HPSounds.menu_sfx.timer_5', SLOT_None, 0.5);
		else
			PlaySound(Sound'HPSounds.menu_sfx.timer_6', SLOT_None, 0.5);
		fLastTickTime = fCountdownTime;
	}
}

auto state Idle
{
	// If triggered, toggle to start or stop.
	event Trigger(actor Other, pawn EventInstigator)
	{
		Level.PlayerHarryActor.ClientMessage("countdown ON");
		StartCountdown();
	}
}

state CountingDown
{
	// On Tick, see if time has ended.  If it has, stop it and sent out an event.
	event Tick(float fDelta)
	{
		if (!HPHud((Level.PlayerHarryActor).myHud).IsCutSceneOrPopupInProgress())
			fCountdownTime -= fDelta;

		// If reached the end.
		if (fCountdownTime <= 0.0)
		{
			StopCountdown();

			// Let anyone who cares know that the time is up.
			TriggerEvent(Event, none, none );
		}
	}

	// If triggered, stop countdown
	event Trigger(actor Other, pawn EventInstigator)
	{	
		Level.PlayerHarryActor.ClientMessage("countdown off");
		StopCountdown();
	}

	function RenderHudItemManager(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
		if (bMenuMode)
			return;

		DrawCountdown(canvas);
		PlayCountdownSound();
	}

	event BeginState()
	{
		fCountDownTime = GetTimerDuration();
		fLastTickTime  = 0;
	}
}

defaultproperties
{
	DrawType=DT_Sprite							// For editor drawing
	bHidden=true                                // Displays in editor, but not game
	CutName="CountdownTimerManager"
}

