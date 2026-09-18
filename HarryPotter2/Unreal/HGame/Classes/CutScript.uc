class CutScript expands actor;
	
var (Script) string Script;
var int curScriptPosition;

const MAX_CUES=150;
var string aCues[150];//MAX_CUES			//array of recieved cues.
var string aPendingCues[150];//MAX_CUES	//array of cues we are waiting for.
var int nPendingCues;
var int nCues;
var int lastCueNum;	//used as a static varible.

var string pendingTimerCue;	//cue to issue on timer event. Used by WAITFOR durration.

const MAX_CAPTURED_ACTORS=30;
var actor aCapturedActors[30];//MAX_CAPTURED_ACTORS

var bool bPlaying;	//true if script is currently running.
var CutScene parentCutScene;

var bool bFastForward;

function bool CaptureActor(string actName)
{
local int i;
local actor act;

	foreach AllActors(class 'actor',act)
		{
		if(act.CutName~=actName)
			break;
		}

	if(act==None)
		{
		CutError("Can't Capture " $actName $". Actor not found.");
		return false; 
		}

	if( act.CutNotifyActor!=None)
		{
		CutError("Can't Capture " $actName $". Actor already captured by" $act.CutNotifyActor $".");
		return false; 
		}

	for(i=0;i<MAX_CAPTURED_ACTORS;i++)
		{
		if(aCapturedActors[i]==None)
			{
			if( act.CutCommand("CAPTURE") )
				{
				act.CutNotifyActor=self;
				aCapturedActors[i]=act;

				//This'll be one of the few places in the game where TickParent2 is used.  This makes sure the CutScene is updated before the BaseCam.
				if( act.IsA('BaseCam') )
					act.TickParent2 = self;

				return(true);
				}
			else
				{
				CutError("Failed to Capture " $actName $". Actor refused capture.");
				return false; 
				}

			}
		}
	CutError("Failed to Capture " $actName $". Too many captured actors.");
	return(false);
}

	//special case for the camera. Allows the camera to be ReCaptured in different threads.
function bool ReCaptureActor(string actName)
{
local int i;
local actor act;

	foreach AllActors(class 'actor',act)
		{
		if(act.CutName~=actName)
			break;
		}

	if(act==None)
		{
		CutError("Can't ReCapture " $actName $". Actor not found.");
		return false; 
		}

	if( act.CutNotifyActor==None)
		{
		CutError("Can't ReCapture " $actName $". Actor not already captured.");
		return false; 
		}

	for(i=0;i<MAX_CAPTURED_ACTORS;i++)
		{
		if(aCapturedActors[i]==None)
			{
			act.CutNotifyActor=self;
			aCapturedActors[i]=act;

			//This'll be one of the few places in the game where TickParent2 is used.  This makes sure the CutScene is updated before the BaseCam.
			if( act.IsA('BaseCam') )
				act.TickParent2 = self;

			return(true);
			}
		}
	CutError("Failed to ReCapture " $actName $". Too many captured actors.");
	return(false);
}


function bool ReleaseActor(string actName)
{
local int i;
	for(i=0;i<MAX_CAPTURED_ACTORS;i++)
		{
		if(aCapturedActors[i]!=None && aCapturedActors[i].CutName~=actName)
			{
			if( aCapturedActors[i].CutCommand("RELEASE") )
				{
				if(aCapturedActors[i].CutNotifyActor!=self)
					{
						//should never happen.
					CutError("Failed to Release " $actName $". Actor Notify doesn't match!?!?!?!.");
					return false; 
					}
				aCapturedActors[i].CutNotifyActor=None;
				aCapturedActors[i]=None;
				return(true);
				}
			else
				{
					//dont think this should ever happen.
				CutError("Failed to Release " $actName $". Actor refused Release!?!?!?!.");
				return false; 
				}
			}
		}

	CutError("Failed to Release " $actName $". Actor not captured.");
	return(false);

}

function actor FindCutSubject(string subjectName)
{
local int i;

	if(subjectName~="baseCam")		//special case so camera can be used from any thread.
	{
		return(harry(level.playerHarryActor).cam);
	}


	for(i=0;i<MAX_CAPTURED_ACTORS;i++)
		{
		if(aCapturedActors[i]!=None && aCapturedActors[i].CutName~=subjectName)
			{
				//found
			return(aCapturedActors[i]);
			}
		}

	CutError("Failed to find " $subjectName $". Actor not captured.");
	return(none);	//failed to find actor.
}
function CutLog(string str)
{

//	HPConsole(level.PlayerHarryActor.player.console).CutConsoleLog(str);

	level.PlayerHarryActor.ClientMessage("CutLog:"$str);
	log("CutLog:"$str);
}
function CutError(string str)
{
	HPConsole(level.PlayerHarryActor.player.console).CutConsoleLog("**** Error:"$str);

	level.PlayerHarryActor.ClientMessage("**** CutError:"$str);
	log("**** CutError:"$str);
}


function bool GetNextLine(out string line)
{
local int eolIndex;
local string tempStr;

		//see if we are at end of script.
	if(curScriptPosition>=Len(script))
		return(false);		//no more lines.

		//get script from cur postion to the end.
	tempStr=Mid(script,curScriptPosition);

		//find eol char
	eolIndex=instr(tempStr,Chr(0x0d));

	if(eolIndex<0)	//if no eol char then end must be at end of string.
		eolIndex=Len(tempStr);

		//update curPostion. bypass line plus 0d and 0a
	curScriptPosition+=eolIndex+2;	

	line=Left(tempStr,eolIndex);	//return working part of the line.

	return(true);
	
}


function bool GetNextCommand(out string command)
{
local string line;
local int commentIndex;
local int OtherCommentIndex;

	if(!GetNextLine(line))
		return false;

	//debugging
//	CutLog("CUTLINE STRING:"$line);

	//strip out comments
	commentIndex=instr(line,";");

		//Added Support for C++ style comments.
	otherCommentIndex=instr(line,"/");
	if( otherCommentIndex>-1)
		if(commentIndex<0 || otherCommentIndex<commentIndex)
			commentIndex=OtherCommentIndex;


	if(commentIndex>-1)
		command=Left(line,commentIndex);
	else
		command=line;

	//debugging
//	CutLog("CUTCOMMAND STRING:"$command);

	//success
	return(true);	
	
}




function CutCue(string cue)
{
local int i;

	if(cue=="")
		CutError("Error blank cue");

	aCues[nCues]=cue;
	nCues++;

	if(nCues>MAX_CUES)
		CutError("Failed to CutCue. Exceeded MAX_CUES");	//not a fatal error (yet).

//	CutLog(self $"Got cue:" $cue);

	if(nPendingCues>0)
		{
		//check to see if cue matches pending cue
		for(i=0;i<nPendingCues;i++)
			{
			if(aPendingCues[i]~=cue)
				{
				//remove pending cue
				if(nPendingCues-1==i) //if this is the last cue
					{
					//just clear and decrement.
					aPendingCues[i]="";
					nPendingCues--;
					}
				else	//if not last pending cue
					{
					//move last cue to this slot.
					aPendingCues[i]=aPendingCues[nPendingCues-1];
					nPendingCues--;
					}
				}
			}	
		}

}

function AddPendingCue(string cue)
{
local int i;

	if(cue=="")
		CutError("Error blank Pending cue");

		//see if cue has already arrived.
	for(i=0;i<nCues;i++)
		if(aCues[i]~=cue)
			return;		//already recieved cue so do nothing.

	aPendingCues[nPendingCues]=cue;
	nPendingCues++;

//	CutLog("Added Pending Cue:" $aPendingCues[nPendingCues-1] $" " $cue );

	if(nPendingCues>MAX_CUES)
		CutError("Failed to AddPendingCue. Exceeded Pending MAX_CUES");	//not a fatal error (yet).
}

function string GenerateUniqueCue()
{
	default.lastCueNum++;
	return("_" $default.lastCueNum  $"UniqueCue");
}




function bool ParseCommand(string command)
{
local string subjectPart;
local string actionPart;
local string untilCue;
 
local actor subjectActor;
local BossEncounterTrigger aTrigger;

local int index,eow;

local int i;
local float timerDuration;
local string questionString;

local name   n;

/*	//Debugging
	log("ParseCommand Words:");
	for(i=0;i<5;i++)
		log(i $" "$ParseDelimitedString(command," ",i,false));

*/
		//Subject is first word
	subjectPart=ParseDelimitedString(command," ",1,false);

	if(	len(subjectPart)==0 )
		{
//		CutLog("Empty Line");
		return(true);	//probably not an error.
		}

	//see if this is a conditional line.
	if(subjectPart~="IF")
		{
		questionString=ParseDelimitedString(command," ",2,false);
		if(!level.playerHarryActor.CutQuestion(questionString))
			{//question was answered false
				//see if it was an error.
			if(level.playerHarryActor.CutErrorString!="")
				CutError(level.playerHarryActor.CutErrorString);
			return(true);//done processing.
			}
		else
			{	//question was answered true
					//bypass conditional
				command=Mid(command,instr(command,questionString)+len(questionString)+1);
					//get new subject
				subjectPart=ParseDelimitedString(command," ",1,false);
			}

		}
	if(subjectPart~="IFNOT")
		{
		questionString=ParseDelimitedString(command," ",2,false);
		if(level.playerHarryActor.CutQuestion(questionString))
			{//question was answered false
				//see if it was an error.
			if(level.playerHarryActor.CutErrorString!="")
				CutError(level.playerHarryActor.CutErrorString);
			return(true);//done processing.
			}
		else
			{	//question was answered true
					//bypass conditional
				command=Mid(command,instr(command,questionString)+len(questionString)+1);
					//get new subject
				subjectPart=ParseDelimitedString(command," ",1,false);
			}

		}

		//see if subject is a keyword
	switch(subjectPart)
		{
		case "CUE":
			CutLog("Issuing: CUE:" $ParseDelimitedString(command," ",2,false));
			parentCutScene.CutCue( ParseDelimitedString(command," ",2,false) );

			return(true);
			break;
		case "WAITFOR":
			CutLog("Issuing: WAITFOR:" $ParseDelimitedString(command," ",2,false));
			AddPendingCue(ParseDelimitedString(command," ",2,false));
			return(true);
			break;
		case "SLEEP":
			if(bFastForward)
				return(true);
			CutLog("Issuing: SLEEP:" $ParseDelimitedString(command," ",2,false));
			pendingTimerCue=GenerateUniqueCue();
			timerDuration=float(ParseDelimitedString(command," ",2,false));
			if(timerDuration>0 && timerDuration<100)
				{
				AddPendingCue(pendingTimerCue);
				settimer(timerDuration,false);
				}
			else
				{
				CutError("Invalid SLEEP duration:" $timerDuration);
				}
			return(true);
			break;
		case "CAPTURE":
			CutLog("Issuing: CAPTURE " $ParseDelimitedString(command," ",2,false));
			CaptureActor(ParseDelimitedString(command," ",2,false));
			return(true);
			break;
		case "RECAPTURE":
			CutLog("Issuing: RECAPTURE " $ParseDelimitedString(command," ",2,false));
			ReCaptureActor(ParseDelimitedString(command," ",2,false));
			return(true);
			break;
		case "RELEASE":
			CutLog("Issuing: RELEASE " $ParseDelimitedString(command," ",2,false));
			ReleaseActor(ParseDelimitedString(command," ",2,false));
			return(true);
			break;
		case "TRIGGER":
			CutLog("Issuing: TRIGGER " $ParseDelimitedString(command," ",2,false));
			TriggerEvent(name(ParseDelimitedString(command," ",2,false)), self, harry(level.playerharryactor) );
			return(true);
			break;
		//case "TRIGGERBOSSENCOUNTER":  //This is a hack, cause for some reason "TRIGGER" doesn't work.  Wish I had time to figure out why.
		//	n = name(ParseDelimitedString(command," ",2,false));
		//	CutLog("Issuing: TRIGGERBOSSENCOUNTER "$n);
		//	ForEach AllActors(class'BossEncounterTrigger', aTrigger, n)
		//	{
		//		//Calling Trigger doesn't work, nothing happens...
		//		aTrigger.TriggerToo( self, harry(level.playerharryactor) );
		//		Log("************* Found Trigger:"$aTrigger$" trigger.tag:"$aTrigger.tag);
		//	}
		//
		//	return(true);
		//	break;
		case "CAMERASHAKE": //CameraShake [magnitude] [time=n]
			CutLog("Issuing: CameraShake " $ParseDelimitedString(command," ",2,false));
			CutCommand_CameraShake( ParseDelimitedString(command," ",2,false) );
			return(true);
			break;
		case "DISABLEPLAYERINPUT":
			CutLog("Issuing: DisablePlayerInput");
			harry(level.playerharryactor).DisablePlayerInput();
			return(true);
			break;
		case "ENABLEPLAYERINPUT":
			CutLog("Issuing: EnablePlayerInput");
			harry(level.playerharryactor).EnablePlayerInput();
			return(true);
			break;
		case "MESSAGE":
			CutLog(ParseDelimitedString(command," ",2,true));
			return(true);
			break;
		case "COMMENT":
			HPHud(Harry(level.PlayerHarryActor).myHud).managerCutScene.SetCutCommentText(ParseDelimitedString(command," ",2,true));
			return(true);
			break;
		case "SLOMO":
			Harry(level.PlayerHarryActor).SloMo(float(ParseDelimitedString(command," ",2,true)));
			CutLog("Issuing: SloMo " $float(ParseDelimitedString(command," ",2,true)));
			return(true);
			break;
		case "SAVEGAME":
			Harry(level.PlayerHarryActor).SaveGame(0);
			CutLog("Issuing: SaveGame ");
			return(true);
		case "CHANGEGAMESTATE":
			CutLog("Issuing: SetGameState " $ParseDelimitedString(command," ",2,true) );
			if( !Harry(level.PlayerHarryActor).SetGameState( ParseDelimitedString(command," ",2,true) ) )
			{
				CutLog( "!E!R!R!O!R! GameState " $ParseDelimitedString(command," ",2,true) $" is not a valid GameState in the *GameStateMasterList*!!!" );
			}
			return(true);
		default:
			break;

		}
	if( subjectPart ~= "SetObjectiveId")
	{
		Harry(level.PlayerHarryActor).SetObjectiveTextId(ParseDelimitedString( command, " ", 2, false ));
		return true;
	}

		//lookup  subject
	subjectActor=FindCutSubject(subjectPart);
	if(subjectActor==None)
		{
			//invalid subject.
		CutError("Failed to Parse Command:" $command $". Can't find subject");
		return false;
		}


		//Action is everthing inbetween subject and end of line or *
	actionPart=ParseDelimitedString(command," ",2,true);

		//Find nowait flag (if any).
	index=instr(actionPart,"*");
	if(index<0)
		{
		untilCue=GenerateUniqueCue();
		AddPendingCue(untilCue);
		}
	else
		{
			//cue name follows *
		untilCue = ParseDelimitedString( Mid( actionPart, index + 1 ), " ", 1, false );

			//if no specified cue make one up.
		if(len(untilCue)==0)
			untilCue=GenerateUniqueCue();

		actionPart=Left(actionPart,index);
		}

	//Try and pull spaces off the end of the actionPart
	while( Asc(Right(actionPart, 1)) == 32  ||  Asc(Right(actionPart, 1)) == 9 )
		actionPart = Left(actionPart, len(actionPart) - 1);

	while( Asc(Right(untilCue, 1)) == 32  ||  Asc(Right(untilCue, 1)) == 9 )
		untilCue = Left(untilCue, len(untilCue) - 1);

	CutLog("Issuing to "$subjectActor.CutName$":" $actionPart $" Cue:" $untilCue);
	if(!subjectActor.CutCommand(actionPart,untilCue,bFastForward))
		{
		CutError("Command Error:" $subjectActor.CutErrorString $" From:"$subjectActor);
		subjectActor.CutErrorString="";	//clear the error message.
		}


}

//*************************************************************************************************************************
function bool CutCommand_CameraShake(string command)
{
	local actor        a;
	local string       sString;
	local int          i;
	local float        magnitude;
	local float        time;

	magnitude = 100;
	time = 0.5;
	i = 2;

	for( i=i; i < 20; i++ )
	{
		sString = ParseDelimitedString( command, " ", i, false );

		if( sString == "" )
			break;
		else
		if( float(sString) > 0 )
			magnitude = float(sString);
		else
		if( Left(sString, len("time=")) ~= "time=" )
			time = float( Mid(sString, len("time=")) );
	}

	//Do your stuff...
	harry(level.playerharryactor).ShakeView( time, magnitude, magnitude );

	return true;
}

//*************************************************************************************************************************
function Play()
{

	GotoState('Running');
}

function Pause()
{
	GotoState('Idle');
}

function FastForward()
{
local TimedCue tc;
		//force all timed cues to expire right now.
	foreach allActors(class'TimedCue',tc)
		{
		if(tc.CutNotifyActor==self)
			{
			tc.Timer();
			}
		}
	bFastForward=true;
}

function Reset()
{
	//clear out cues

	//reset varibles

	
}

auto state idle
{
	Begin:
		bPlaying=false;
}


state Running
{
	function BeginState()
		{
		bPlaying=true;
		bFastForward=false;
		}

	event Timer()
		{
		if(pendingTimerCue!="")
			{
			CutCue(pendingTimerCue);
			pendingTimerCue="";
			}
		}

	event Tick(float deltaTime)
		{
		local string command;

		while(nPendingCues==0)
			{
			if(!GetNextCommand(command))
				{
				//No more commands.
		
				CutLog(self $" Finishing");

				GotoState('Finished');
				return;
				}

			if(!ParseCommand(command))
				{
				//error processing command
				}
			}
		}



				

}


state Finished
{
	Begin:
		bPlaying=false;
}
defaultProperties
{
	bHidden=true;
	bShadowCast=false;
	bCollideActors=false;
	bBlockActors=false;
	bBlockPlayers=false;

	bPlaying=false;

}