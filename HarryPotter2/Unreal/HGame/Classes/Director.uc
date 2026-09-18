//=============================================================================
// Director -- Controller of a mini-game, puzzle, etc.; base class
//=============================================================================
class Director extends Actor;

// This class is the base class for all Mini-Game Director classes.  It is an
// abstract class.  Define your own director class based on this one and place
// an instance of that class into the level map for the mini-game it is for.

// All mini-game directors must be tagged as 'Director' (which is set as
// default by this class, but check it anyway) so that event detectors in the
// level know who to trigger without having to know exactly what mini-game is
// being played.

// Objects that detect touch events that could be important to the overall game
// state, must report the event to the Director by calling the OnTouchEvent
// function.  Descendant director's override that handler and respond to the
// touch event as appropriate for the type of mini-game.  Same goes for Bump
// events and others.

// Standard trigger objects communicate to the director by having 'Director'
// as their trigger event.  These events are caught by the OnTriggerEvent
// handler, which descendant director's override and respond to the event as
// appropriate.  Note that the director cannot distinguish which trigger object
// triggered a 'Director' event.

// CutScene's can communicate to the director via the CutCommand interface.
// These commands are handled by the CutCommand handler, which descendant
// directors override and respond to the commands as appropriate.

// Alternatively, CutScene's can communicate to the director by using their
// Trigger command and 'Director' as the trigger event.  These events are
// caught by the OnCutSceneEvent handler, which descendant directors override
// and respond to the event as appropriate.

// CutScene's can also ask questions of the director.  Cut-questions are always
// aimed at Harry, but Harry will give the Director first-shot at answering it;
// unanswered questions are routed back to Harry.

// All descendant Directors can use the PlayerHarry variable to channel
// ClientMessages through.

var Harry		PlayerHarry;
var baseConsole	Console;

function PreBeginPlay()
{
	// Initialize
	Super.PreBeginPlay();

	// Find player that channels ClientMessages
	foreach AllActors( class'Harry', PlayerHarry )
		break;
}

function OnTouchEvent( Pawn Subject, Actor Object )
{
	// Something touched something, and the event affects the flow of the
	// mini-game.  Update game state accordingly.

	// This is the default handler for mini-game touch events; the descendant class
	// should override this function to handle all mini-game touch events that the
	// director needs to know about, but it should also call this version for any
	// touch events that the descendant class doesn't know how to handle.

	// Unexpected touch event
	PlayerHarry.ClientMessage( Subject.Name$" touched "$Object.Name );
}

function OnUnTouchEvent( Pawn Subject, Actor Object )
{
	// Something untouched something, and the event affects the flow of the
	// mini-game.  Update game state accordingly.

	// This is the default handler for mini-game untouch events; the descendant class
	// should override this function to handle all mini-game untouch events that the
	// director needs to know about, but it should also call this version for any
	// untouch events that the descendant class doesn't know how to handle.

	// Unexpected untouch event
	PlayerHarry.ClientMessage( Subject.Name$" untouched "$Object.Name );
}

function OnBumpEvent( Pawn Subject, Actor Object )
{
	// Something bumped something, and the event affects the flow of the
	// mini-game.  Update game state accordingly.

	// This is the default handler for mini-game bump events; the descendant class
	// should override this function to handle all mini-game bump events that the
	// director needs to know about, but it should also call this version for any
	// bump events that the descendant class doesn't know how to handle.

	// Unexpected bump event
	PlayerHarry.ClientMessage( Subject.Name$" bumped "$Object.Name );
}

function OnHitEvent( Pawn Subject )
{
	// Something hit part of the world (walls/floors), and the event affects
	// the flow of the mini-game.  Update game state accordingly.

	// This is the default handler for mini-game touch events; the descendant class
	// should override this function to handle all mini-game hit events that the
	// director needs to know about, but it should also call this version for any
	// hit events that the descendant class doesn't know how to handle.

	// Unexpected hit event
	PlayerHarry.ClientMessage( Subject.Name$" hit an obstacle" );
}

function OnCutSceneEvent( Name CutSceneTag )
{
	// A CutScene triggered the Director.  The CutSceneTag parameter is the
	// tag name of the triggering CutScene.

	// This is the default handler for mini-game CutScene events; the descendant class
	// should override this function to handle all mini-game CutScene trigger events
	// aimed at the director, but it should also call this version for any CutScene
	// events that the descendant class doesn't know how to handle.

	// Unexpected CutScene event
	PlayerHarry.ClientMessage( "CutScene "$CutSceneTag$" triggered Director" );
}

function OnTriggerEvent( Actor Other, Pawn EventInstigator )
{
	// Something activated a standard trigger that was linked to the 'Director'.
	// Update game state accordingly.

	// The Other parameter will be the actor that activated the trigger, and the
	// EventInstigator will be "Other's" current instigator.  The trigger object
	// itself is not identified.

	// This is the default handler for mini-game trigger events; the descendant class
	// should override this function to handle all mini-game trigger events aimed at
	// the director, but it should also call this version for any trigger events that
	// the descendant class doesn't know how to handle.

	// Unexpected trigger event
	PlayerHarry.ClientMessage( Other$" triggered Director with "$EventInstigator );
}

function Trigger( Actor Other, Pawn EventInstigator )
{
	// Something triggered a 'Director' event.  This could originate from an
	// actor encountering a standard trigger object, or a CutScene executing a
	// Trigger command from its script.

	// For standard trigger objects, the Other parameter will be the actor that
	// activated the trigger, and the EventInstigator will be "Other's" current
	// instigator.  The trigger object itself is not identified.

	// For CutScene triggers, the Other parameter will be the CutScene itself,
	// and the EventInstigator will be None.

	// This function simply splits the possibilities into two handlers.

	local CutScene	CutScene;

//	PlayerHarry.ClientMessage( Other$" triggered Director with "$EventInstigator );
//	Log( Other$" triggered Director with "$EventInstigator );
	CutScene = CutScene( Other );

	if ( CutScene != None )
	{
		OnCutSceneEvent( CutScene.Tag );
	}
	else
	{
		OnTriggerEvent( Other, EventInstigator );
	}
}

function bool OnCutCapture()
{
	// A CutScene captured the Director.
	// Descendant classes can override this function to react to a capture event.

	return true;
}

function bool OnCutRelease()
{
	// A CutScene released the Director.
	// Descendant classes can override this function to react to a release event.

	return true;
}

function bool CutCommand( string Command, optional string Cue, optional bool bFastFlag )
{
	// A cut-scene is commanding the director to do something; parse out command and
	// respond accordingly.

	// This is the default handler for mini-game cut-commands; the descendant class
	// should override this function to handle all mini-game cut-commands that need
	// aimed at the director, but it should also call this version for any commands
	// that the descendant class doesn't know how to handle.

	// The common commands of Capture and Release are handled here; they are
	// dispatched to handlers OnCutCapture and OnCutRelease.

	local string	sActualCommand;


	sActualCommand = ParseDelimitedString( Command, " ", 1, false );

	// Dispatch command
	if ( sActualCommand ~= "Capture" )
	{
		return OnCutCapture();
	}
	else if ( sActualCommand ~= "Release" )
	{
		return OnCutRelease();
	}
	else
	{
		// Unknown director cut-command
		PlayerHarry.ClientMessage( "Director received an unknown cut-command" );
		return Super.CutCommand( Command, Cue, bFastFlag );
	}
}

function bool CutQuestion( string Question )
{
	// A cut-scene is asking a question the Director might know the answer to.
	// Harry has given the Director first chance at answering it; the Director
	// either answers the question or passes it back to Harry by setting the
	// CutErrorString to "Unanswered".  If un-answered, Harry will give it a try.

	// This is the default handler for mini-game cut-questions; the descendant class
	// should override this function to handle all mini-game related cut-questions,
	// but it should also call this version for any questions it can't answer.

	CutErrorString = "Unanswered";
	return false;
}

function OnPlayerPossessed()
{
	// Called when player gets possessed by (attached to) the viewport (Player).
	// This is the first moment when the Player member of PlayerPawn is valid,
	// and thus a reference to the Console.

	// Get a reference to the console
	Log( "Player possessed" );
	Console = baseConsole( PlayerHarry.Player.Console );
}

function OnPlayerTravelPostAccept()
{
	// Called when player gets the TravelPostAccept event.  This is the moment
	// when the player's traveling items become valid.

	Log( "Player processed TravelPostAccept event" );
}

function OnPlayerDying()
{
	// Called when player starts dying.
	// Descendant classes can override this function to react to Harry starting to die.

	PlayerHarry.ClientMessage( "Player dying..." );
}

function OnPlayersDeath()
{
	// Called when player dies.
	// Descendant classes can override this function to react to Harry finished dying.

	PlayerHarry.ClientMessage( "Director: Player died" );
}

function OnActionKeyPressed()
{
	// Called when the player's "Action" key/button is pressed.
	// Descendant classes can override this function to react to the action key event.

	PlayerHarry.ClientMessage( "Action key pressed" );
}


defaultproperties
{
	Tag=Director
	Texture=Texture'Engine.S_Flag'
	DrawScale=3.0
	bHidden=true
}
