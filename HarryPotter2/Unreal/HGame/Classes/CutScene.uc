class CutScene expands actor;

const MAX_THREADS=20;
var (CutScene) string aThreadScripts[20];//MAX_THREADS
var CutScript aThreads[20];//MAX_THREADS


// vars that save true/false settings
var bool bHasBeenDisabled;
var bool Saved_bBumpStarts;
var bool Saved_bTriggerStarts;
var bool Saved_bDoImmediateStart;

var (CutScene) bool bBumpStarts;
var (CutScene) bool bTriggerStarts;

var (CutScene) bool bLevelLoadStarts;
var (CutScene) string FileName;

var (CutScene) bool bPlayOnce;
var (CutScene) bool bDelayLevelFadeIn;

var (CutScene) bool bTriggerTogglesBumpStart;

var bool bDoImmediateStart;

var bool bPlaying;

var int numScriptsPlaying;	//NOTE: Only use as default.numScriptsPlaying property.

var int nPlayedCount;		//incremented every time cutscene plays.

function OpenCutConsole()
{
	HPConsole(level.playerharryactor.player.console).ShowCutConsole(true);
}
function CutCue(string cue)
{
	local int i;
	for(i=0;i<ArrayCount(aThreads);i++)
	{
		if(aThreads[i]!=None)
		{
			//REVISIT: More gracefull exit for the threads.
			aThreads[i].CutCue(cue);
		}
	}
	
}

event OnResolveGameState()
{
	// If we are not in the current GameState then turn off collision and disable this cutScene
	if( !bInCurrentGameState )
	{
		// We are NOT in the current state
//		TriggerDisable();
		GotoState('disabled');

		log("*** " $self $" is NOT in the current GameState. CurrentGameState = " 
			$Harry(level.playerharryactor).CurrentGameState );
	}
	else
	{

		GotoState('idle');

		log("*** " $self $" is *IN* the current GameState. CurrentGameState = " 
			$Harry(level.playerharryactor).CurrentGameState );
		
		if(bDelayLevelFadeIn && bLevelLoadStarts)
		{
			if(bPlayOnce && nPlayedCount>0)
				return;
			else
				harry(level.playerharryactor).FadeRate = 0;
		}
	}
}

event PostBeginPlay()
{
	if( bLevelLoadStarts )	//because there is a problem running play from PostBeginPlay.
		bDoImmediateStart = true;
}

function CreateThreads()
{
	local int i,t;
	local string line;
	
	if(FileName!="")
	{
		for(t=0;t<ArrayCount(aThreads);t++)
		{
			//see if this thread exists.
			line=Localize( "thread_"$t, "line_0","Cutscenes\\" $FileName );
			if( instr(line,"<?")==-1)
			{
				aThreads[t]=spawn(class'CutScriptDisk');
				CutScriptDisk(aThreads[t]).load( "thread_"$t,"Cutscenes\\" $FileName );
				aThreads[t].parentCutScene=self;
				aThreads[t].Play();
			}
		}
	}
	else
	{
		for(i=0;i<ArrayCount(aThreads);i++)
		{
			if(len(aThreadScripts[i])>0)
			{
				aThreads[i]=spawn(class'CutScript');
				aThreads[i].script=aThreadScripts[i];
				aThreads[i].parentCutScene=self;
				aThreads[i].Play();
			}
		}
	}
	
}

function DeleteThreads()
{
	local int i;
	
	for(i=0;i<ArrayCount(aThreads);i++)
	{
		if(aThreads[i]!=None)
		{
			//REVISIT: More gracefull exit for the threads.
			aThreads[i].destroy();
		}
	}
}

function bool CheckFinished()
{
	local int i;
	local bool bStillRunning;
	
	bStillRunning=false;
	for(i=0;i<MAX_THREADS;i++)
	{
		if(aThreads[i]!=None && aThreads[i].bPlaying)
		{
			bStillRunning=true;
		}
	}
	return(!bStillRunning);
}

auto state disabled
{
	event BeginState()
	{
		harry(Level.playerHarryActor).clientmessage(self $" Entering DISABLED state");
		if( bInCurrentGameState )
		{
			harry(Level.playerHarryActor).clientmessage(self $" Changing to ENABLED state");
			GotoState('idle');
		}
	}
	event Touch(Actor other)
	{
	}
	
	event Trigger( actor Other, pawn EventInstigator )
	{
	}

	function Play()
	{
	}
}
state idle
{
	event BeginState()
	{
		harry(Level.playerHarryActor).clientmessage(self $" Entering ENABLED state");
		if( !bInCurrentGameState )
		{
			harry(Level.playerHarryActor).clientmessage(self $" Changing to DISABLED state");
			GotoState('disabled');
		}
	}
	
	event Touch(Actor other)
	{
		if(harry(other)!=None)
		{
			harry(other).clientmessage(self $" Bumped. bBumpStarts="$bBumpStarts);
			if(bBumpStarts)
				Play();		
			//	OpenCutConsole();
		}
	}
	
	event Trigger( actor Other, pawn EventInstigator )
	{
		if(bTriggerTogglesBumpStart)
		{
			bBumpStarts=!bBumpStarts;
			harry(Level.playerHarryActor).clientmessage(self $"+++++++++++++++++++ Triggered bTriggerTogglesBumpStart. bBumpStarts="$bBumpStarts);
		}

		if(bInCurrentGameState && bTriggerStarts)
		{
			if(bPlayOnce)
				bTriggerStarts=false;		//disable trigger from now on.
			Play();
		}
	}
	

begin:
	bPlaying=false;	

	if(bDoImmediateStart)	//because there is a problem running play from PostBeginPlay.
	{
		bDoImmediateStart=false;
		sleep(0.20);	//small delay to allow level to fully init.
		Play();
	}


}

state running
{
	event Tick(float time)
	{
		local int i;
		//check for fast forward.
		if(harry(level.PlayerHarryActor).bSkipKeyPressed)
		{ 
			for(i=0;i<MAX_THREADS;i++)
			{
				if(aThreads[i]!=None && aThreads[i].bPlaying)
				{
					aThreads[i].FastForward();
				}
			}
		}
	}
	
begin:
	bPlaying=true;	
	default.numScriptsPlaying++;
		
loop:
		
		if(CheckFinished())
			gotostate('Finished');
		sleep(0.5);
		goto('loop');
}

state finished
{
begin:
bPlaying=false;
default.numScriptsPlaying--;
	DeleteThreads();	
	Harry(level.PlayerHarryActor).SloMo(1.0);	//restore slomo in case it was used.
loop:
	
	if(!bPlayOnce)	//allow cutscene to restart?
		gotostate('idle');
	
}

//overridden in state running
function FastForward()
{
}

function Play()
{
	if(bPlaying)
		return;	//already playing.
	
	if(bPlayOnce && nPlayedCount>0)
		return;	//only play once.
	
	nPlayedCount++;

	log("CutScene " $self $" starting for the " $nPlayedCount $" time" );

	
	CreateThreads();
	
	gotostate('Running');
}

function bool CutCommand(string command,optional string cue,optional bool bFastFlag)
{
local string sActualCommand;
local string targetName;
local actor tempActor;

		//allow cutscenes to be catured so they can teleport.
	sActualCommand = ParseDelimitedString( command, " ", 1, false );
	if( sActualCommand ~= "Capture" )
	{
		return true;
	}
	else
	if( sActualCommand ~= "Release" )
	{
		return true;
	}

	return(super.CutCommand(command,cue,bFastFlag));
}


defaultproperties
{
	bHidden=true;
	bStatic=false;
	bBumpStarts=true;
	
	bPlayOnce=true;
	nPlayedCount=0;
	
	Mesh=skCutSceneMarkerMesh;
    DrawType=DT_Mesh;
    AmbientGlow=75;
	
	
	bShadowCast=false;
	bCollideActors=true;
	bBlockActors=false;
	bBlockPlayers=false;
	
}



