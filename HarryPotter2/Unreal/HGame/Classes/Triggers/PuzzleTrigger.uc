class PuzzleTrigger extends trigger;

var(Puzzle) int		KeyFrames[8];
var(Puzzle) name	Tags[8];
var(Puzzle) name	PuzzleName;

var			float  fWaitTime;
var			float  fCurrTime;

var			bool	bIsOpen;
var			int		Pieces;

//*******************************************************************************
function PostBeginPlay()
{
	local ElevatorMover em;
	local int count;

	Super.PostBeginPlay();

	// puzzle does not have a name, disable it
	if(PuzzleName == '')
	{
		Pieces = 0;
		return;
	}

	count = 0;
	foreach allActors(class'ElevatorMover', em)
	{
		if(em.PuzzleName == PuzzleName)
		{
			count++;
		}
	}

	Pieces = count;
}

function MyOpenDoor( )
{
	if(!bIsOpen)
	{
		MyToggleDoor( );
	 	bIsOpen = true;
	}
}

function MyCloseDoor( )
{
	if(bIsOpen)
	{
		MyToggleDoor( );
	 	bIsOpen = false;
	}
}

function MyToggleDoor( )
{
	local Trigger t;
	local Mover m;

	if( Event != '' )
	{
		foreach AllActors( class 'Trigger', t, Event )
		{
			foreach AllActors( class 'Mover', m, t.Event )
			{
				t.TriggerEvent(t.Event, None, None);
			}
		}
	}
}

function Tick( float DeltaTime )
{
	local ElevatorMover em;
	local int i;
	local int test[16];
	local bool  doOpen;

	Super.Tick(DeltaTime);

	// there is nothing in this puzzle
	if(Pieces == 0)
		return;

	// checking trigger condition every 1 second
	fCurrTime += DeltaTime;
	if(fCurrTime < fWaitTime)
		return;

	fCurrTime = 0.0f;

	// initialize test
	for(i = 0; i < Pieces; i++)
		test[i] = 0;

	foreach AllActors( class'ElevatorMover', em )
	{
		if(em.PuzzleName == PuzzleName)
		{
			for(i = 0; i < Pieces; i++)
			{
				if( (em.Tag != '') && (em.Tag == Tags[i]) )
					break;
			}

			if(i < Pieces)
			{
				if(em.KeyNum == KeyFrames[i])
					test[i] = 1;
			}
		}
	}

	doOpen = true;
	for(i = 0; i < Pieces; i++)
	{
		// if something does not match
		if(test[i] == 0)
			doOpen = false;
	}

	if(	doOpen )
		MyOpenDoor( );
	else
		MyCloseDoor( );
}

//*****************************************************************************
defaultproperties
{
	fWaitTime=1.000000
	fCurrTime=0.000000

	bIsOpen=false
	Pieces=0
	PuzzleName=none
}