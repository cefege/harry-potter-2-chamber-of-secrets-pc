//=============================================================================
// QuidditchCommentator -- Source of comments made during a quidditch game
//=============================================================================
class QuidditchCommentator extends HChar;

var BroomHarry		Harry;

enum QuidComment	// If changed, update size of Comments[] array, and defaultproperties below
{
	QC_None,         //0	
	QC_HasQuaffle,         //1	
	QC_Scores,         //2	
	QC_WinsSlyth,         //3	
	QC_WinsAgain,         //4	
	QC_WinsMatch,         //5	
	QC_WinsCup,         //6	
	QC_TheresTheSnitch,         //7	
	QC_HereComesSeeker,         //8	
	QC_ClosingOnSnitch,         //9	
	QC_ReachingSnitch,         //10
	QC_MissedSnitch,         //11
	QC_CaughtSnitch,         //12
	QC_DontGiveUp,         //13
	QC_SeekerBattle,         //14
	QC_SeekerBattleNeg,         //15
	QC_SeekerBattlePos,         //16
	QC_HarryKickMiss,         //17
	QC_HarryKickHit,         //18
	QC_OppKickMiss,         //19
	QC_OppKickHit,         //20
	QC_BludgerPursuit,         //21
	QC_BludgerPursuit2,         //22
	QC_BludgerMiss,         //23
	QC_BludgerHit,         //24
	QC_Miss,         //25
	QC_Hit,         //26
	QC_HitNearDeath,         //27
	QC_HitDying,         //28
	QC_Dying,         //29
	QC_Dead,         //30
	QC_ReturnToFlight,         //31
	QC_KeeperDives,         //32
	QC_Block,         //33
	QC_Foul,         //34
	QC_Negative,         //35
	QC_Positive,         //36
	QC_SigningOff,         //37
	QC_InTrenchRun,         //38
	QC_IdleClean,         //39
	QC_IdleRough,         //40
	QC_IdleNoFalls,         //41
	QC_IdleTrickySnitch,         //42

	QC_NumComments
};
const MAX_QUID_COMMENT_NAMES =60;
var string QuidCommentNames[60];//MAX_QUID_COMMENT_NAMES


enum HouseAffiliation	// Must be in same order as in QuidditchReferee.uc
{
	HA_Gryffindor,		// House 0 (also used for generic house)
	HA_Ravenclaw,		// House 1 (also used for generic opponent)
	HA_Hufflepuff,		// House 2
	HA_Slytherin,		// House 3

	HA_Neutral,			// Used when the specific house isn't important
	HA_Opponent,			// Used to refer to the house of the opponent when the specific house isn't important

	HA_NumHouses
};
const MAX_QUID_COMMENT_HOUSE_NAMES =6;
var string QuidCommentHouseNames[6];//MAX_QUID_COMMENT_HOUSE_NAMES



enum TeamAffiliation	// What team a comment refers to; must be in same order as in QuidditchReferee.uc
{
	TA_Gryffindor,
	TA_Opponent,
	TA_Neutral,

	TA_NumAffiliations
};

const QC_MAX_COMMENT_VARIANTS = 30;			// If this changes, change Variants[] in struct VarCommentInfo too

struct CommentInfo {
	var String			DlgName;			// Name used to lookup dialog assets for comment
	var Sound			DlgSound;			// Sound asset for comment
	var String			DlgText;			// Display text for comment
	var bool			bHasBeenSaid;		// Has this comment already been said before
	var float			fTimeLastSaid;		// When was it last said
};

struct VarCommentInfo {
	var CommentInfo		Variant[30];			// All interchangable variations of this comment
	var int				Variations;			// How many variations of this comment are there
	var bool			bHasBeenSaid;		// Has any variant of this comment already been said before
	var float			fTimeLastSaid;		// When was any variant last said
};

struct HouseDependentCommentInfo {
	var VarCommentInfo	House[4];			// House-specific variations of this comment
	var bool			bHasBeenSaid;		// Has any house variant of this comment already been said before
	var float			fTimeLastSaid;		// When was any variant last said
};


var HouseDependentCommentInfo	Comments[29];	// Info on all the comments the quidditch commentator can make

var HouseAffiliation	eOpponent;					// Which house is the opponent team affiliated with
var float				fNextTimeACommentCanBeSaid;	// When is it safe to say another comment
var float				fGapTime;					// How much silence to put between comments

const fNoGapTimeBetweenComments  = 0.1;		// Sliver of time between end of last comment and beginning of next when no gap is desired
const fMinGapTimeBetweenComments = 1.0;		// Lowest Minimum time required between end of last comment and beginning of next
const fMaxGapTimeBetweenComments = 2.4;		// Highest Minimum time required between end of last comment and beginning of next

const fMinTimeBeforeCommentRepeat = 45.0;	// How much time must elapse before a certain comment can be said again


//-------------------------------------------------------------------------------------------
// PostBeginPlay()
//-------------------------------------------------------------------------------------------

function PostBeginPlay()
{
	local int	Variant;

	// Initialize commentator
	Super.PostBeginPlay();

	// Find Harry
	foreach AllActors( class'BroomHarry', Harry )
		break;

	fNextTimeACommentCanBeSaid = 0.0;
	fGapTime = 0.0;

		//load sound names from QuidSet.int
	fillCommentArray();
}

//-------------------------------------------------------------------------------------------
// Dialog functions
//-------------------------------------------------------------------------------------------

function SetOpponent( HouseAffiliation eNewOpponent )
{
	eOpponent = eNewOpponent;
	Log( "QuidditchCommentator: Opponent = "$eOpponent$"." );
}

function float TimeLeftUntilSafeToSayAComment( optional bool bNoGap )
{
	local float	fTimeLeft;

	fTimeLeft = fNextTimeACommentCanBeSaid - Level.TimeSeconds + 0.1;
	if ( bNoGap )
		fTimeLeft += fNoGapTimeBetweenComments;
	else
		fTimeLeft += fGapTime;

	return fTimeLeft;
}

function bool CommentHasBeenSaidBefore( QuidComment eComment )
{
	// Returns true if the any variant of the indicated comment has ever been
	// said before.

	return Comments[ eComment ].bHasBeenSaid;
}

function bool SayComment( QuidComment eComment, optional TeamAffiliation eTeam, optional bool bNoGap )
{
	// Makes the quidditch commentator say the indicated comment. Will
	// automatically choose among available interchangable variants.  Will
	// only pick from variants appropriate for indicated team affiliation (if
	// not generic).  May not say the comment if it has been said too recently
	// or the last one (plus a gap) hasn't finished yet.  If bNoGap is true,
	// then the silence gap after last comment doesn't have to be finished.
	// Returns True if comment was actually said.

	local bool				bSaid;
	local HouseAffiliation	eHouse;

	local int				Variant;
	local int				Tied;
	local int				OldestVariant;
	local float				OldestTime;

	local Sound				DlgSound;

	bSaid = false;
	if ( eComment == QC_None )
		return false;

	// Skip comment if too soon to say another one
	if ( bNoGap )
	{
		if ( Level.TimeSeconds < fNextTimeACommentCanBeSaid + fNoGapTimeBetweenComments )
			return false;
	}
	else
	{
		if ( Level.TimeSeconds < fNextTimeACommentCanBeSaid + fGapTime )
			return false;
	}

	// Determine which house the comment is related to
	switch ( eTeam )
	{
		case TA_Gryffindor:	eHouse = HA_Gryffindor;	break;	// Also used for default, non-team-specific comments
		case TA_Opponent:	eHouse = eOpponent;		break;
		case TA_Neutral:	eHouse = HA_Gryffindor;	break;	// Equal to HA_Neutral
	};

	// See if a house-specific comment is available; if not, look for a generic one
//	if ( Comments[ QC_HitNearDeath ].House[ eHouse ].Variant[ 0 ].DlgName == "" )
	if ( Comments[ eComment ].House[ eHouse ].Variant[ 0 ].DlgName == "" )
	{
		if ( eTeam == TA_Opponent )
		{
			// Try a generic opponent
			eHouse = HA_Ravenclaw;		// Equal to HA_Opponent
			if ( Comments[ eComment ].House[ eHouse ].Variant[ 0 ].DlgName == "" )
			{
				// Try a generic house
				Log( "QuidditchCommentator: Warning: No opponent-specific comment available for type "$eComment$" comment, house "$eOpponent$"." );
				eHouse = HA_Gryffindor;	// Equal to HA_Neutral
				if ( Comments[ eComment ].House[ eHouse ].Variant[ 0 ].DlgName == "" )
				{
					Log( "QuidditchCommentator: Could not find a type "$eComment$" comment for house "$eOpponent$"." );
					return false;
				}
			}
		}
		else
		{
			Log( "QuidditchCommentator: Could not find a type "$eComment$" comment for team "$eTeam$"." );
			return false;
		}
	}

	// Pick a variant (find oldest one; randomize on ties)
	OldestVariant = 0;
	OldestTime = Level.TimeSeconds;
	Tied = 0;
	Variant = 0;
	while (    Variant < QC_MAX_COMMENT_VARIANTS
		    && Comments[ eComment ].House[ eHouse ].Variant[ Variant ].DlgName != "" )
	{
//		Log( "QuidditchCommentator: Checking dialog for type "$eComment$" comment, variant "$Variant$"; DlgName = "
//			 $Comments[ eComment ].House[ eHouse ].Variant[ Variant ].DlgName$"; HasBeenSaid = "
//			 $Comments[ eComment ].House[ eHouse ].Variant[ Variant ].bHasBeenSaid$"; TimeLastSaid = "
//			 $Comments[ eComment ].House[ eHouse ].Variant[ Variant ].fTimeLastSaid$"." );

		if ( Comments[ eComment ].House[ eHouse ].Variant[ Variant ].bHasBeenSaid )
		{
			if ( Comments[ eComment ].House[ eHouse ].Variant[ Variant ].fTimeLastSaid < OldestTime )
			{
				OldestVariant = Variant;
				OldestTime = Comments[ eComment ].House[ eHouse ].Variant[ Variant ].fTimeLastSaid;
			}
		}
		else
		{
			// Never been said before; randomly break tie among others not said before
			++Tied;
			if ( FRand() <= 1.0/Tied )	// Iteratively gives even weight across all choices so far
			{
				OldestVariant = Variant;
				OldestTime = -1.0;
			}
		}

		++Variant;
	}
	Comments[ eComment ].House[ eHouse ].Variations = Variant;	// Remember count
	Variant = OldestVariant;

	// Say the comment (if not too soon to repeat it)
	if (    !Comments[ eComment ].House[ eHouse ].Variant[ Variant ].bHasBeenSaid
		 || ( Level.TimeSeconds > Comments[ eComment ].House[ eHouse ].Variant[ Variant ].fTimeLastSaid + fMinTimeBeforeCommentRepeat ) )
	{
//		if ( Comments[ eComment ].House[ eHouse ].Variant[ Variant ].DlgSound == None )		/***/
//			Harry.TheNarrator.FindDialog( Comments[ eComment ].House[ eHouse ].Variant[ Variant ].DlgName,
//										  Comments[ eComment ].House[ eHouse ].Variant[ Variant ].DlgSound,
//										  Comments[ eComment ].House[ eHouse ].Variant[ Variant ].DlgText );

		DlgSound = Comments[ eComment ].House[ eHouse ].Variant[ Variant ].DlgSound;

		if ( DlgSound != None )
		{
			PlaySound( DlgSound, SLOT_Talk, , , 10000.0 );	// Radius makes sure commentator can be heard far away
			bSaid = true;

			// Mark when this comment was said
			Comments[ eComment ].House[ eHouse ].Variant[ Variant ].fTimeLastSaid = Level.TimeSeconds;
			Comments[ eComment ].House[ eHouse ].fTimeLastSaid = Level.TimeSeconds;
			Comments[ eComment ].fTimeLastSaid = Level.TimeSeconds;

			Comments[ eComment ].House[ eHouse ].Variant[ Variant ].bHasBeenSaid = true;
			Comments[ eComment ].House[ eHouse ].bHasBeenSaid = true;
			Comments[ eComment ].bHasBeenSaid = true;

			// Figure out when its safe to say another comment
			fNextTimeACommentCanBeSaid = Level.TimeSeconds + GetSoundDuration( DlgSound );
			fGapTime = FRand() * (fMaxGapTimeBetweenComments-fMinGapTimeBetweenComments) + fMinGapTimeBetweenComments;

//			Log( "QuidditchCommentator: Said dialog for type "$eComment$" comment; DlgName = "
//				 $Comments[ eComment ].House[ eHouse ].Variant[ Variant ].DlgName$"." );
		}
		else
		{
			Log( "QuidditchCommentator: Failed to say dialog for type "$eComment$" comment; DlgName = "
				 $Comments[ eComment ].House[ eHouse ].Variant[ Variant ].DlgName$"." );
		}
	}
	else
	{
//		Log( "QuidditchCommentator: Skipping dialog for type "$eComment$" comment, variant "$Variant$"; DlgName = "
//			 $Comments[ eComment ].House[ eHouse ].Variant[ Variant ].DlgName$"." );
	}

	return bSaid;
}

function string EventNumToEventName(int num)
{
	return(QuidCommentNames[num]);
}
function string GetCommentId(int eventNum,int house,int variant)
{
local string key;
local string eventName;
local string id;

	eventName=EventNumToEventName(eventNum);

	key=QuidCommentHouseNames[house]$"_"$eventName;

	id=Localize( key,"line"$variant,"QuidSet" );

//	log("*** looking up QuidCommentary("$eventNum$","$house$","$variant$")->Found:"$id);
 
		//check to see if line actually exists.
	if(instr(id,"<")>-1)
		return("");	//doesnt exist
	else
		return(id);

}

function fillCommentArray()
{
local int c,h,v;
local string sndId;

local QuidComment vvv;

	for(c=0;c<MAX_QUID_COMMENT_NAMES;c++)
	{
		for(h=0;h<MAX_QUID_COMMENT_HOUSE_NAMES;h++)
		{
			for(v=0;v<QC_MAX_COMMENT_VARIANTS;v++)
			{
				sndId=GetCommentId(c,h,v);
				if(sndId!="")
				{
					Comments[c].House[h].Variant[v].DlgName=sndId;
					Comments[c].House[h].Variant[v].DlgSound=Sound( DynamicLoadObject("AllDialog."$sndId, class'Sound') );
				}
			}

		}

	}
}

defaultproperties
{
	Texture=Texture'Engine.S_Flag'
	bHidden=true
	eOpponent=HA_Ravenclaw

	QuidCommentHouseNames(0)="QCG";         //0	
	QuidCommentHouseNames(1)="QCH";         //1	
	QuidCommentHouseNames(2)="QCR";         //2	
	QuidCommentHouseNames(3)="QCS";         //3	
	QuidCommentHouseNames(4)="QCN";         //4	
	QuidCommentHouseNames(5)="QCO";         //5	


	QuidCommentNames(0)="None";         //0	
	QuidCommentNames(1)="HasQuaffle",         //1	
	QuidCommentNames(2)="Scores",         //2	
	QuidCommentNames(3)="WinsSlyth",         //3	
	QuidCommentNames(4)="WinsAgain",         //4	
	QuidCommentNames(5)="WinsMatch",         //5	
	QuidCommentNames(6)="WinsCup",         //6	
	QuidCommentNames(7)="TheresTheSnitch",         //7	
	QuidCommentNames(8)="HereComesSeeker",         //8	
	QuidCommentNames(9)="ClosingOnSnitch",         //9	
	QuidCommentNames(10)="ReachingSnitch",         //10
	QuidCommentNames(11)="MissedSnitch",         //11
	QuidCommentNames(12)="CaughtSnitch",         //12
	QuidCommentNames(13)="DontGiveUp",         //13
	QuidCommentNames(14)="SeekerBattle",         //14
	QuidCommentNames(15)="SeekerBattleNeg",         //15
	QuidCommentNames(16)="SeekerBattlePos",         //16
	QuidCommentNames(17)="HarryKickMiss",         //17
	QuidCommentNames(18)="HarryKickHit",         //18
	QuidCommentNames(19)="OppKickMiss",         //19
	QuidCommentNames(20)="OppKickHit",         //20
	QuidCommentNames(21)="BludgerPursuit",         //21
	QuidCommentNames(22)="BludgerPursuit2",         //22
	QuidCommentNames(23)="BludgerMiss",         //23
	QuidCommentNames(24)="BludgerHit",         //24
	QuidCommentNames(25)="Miss",         //25
	QuidCommentNames(26)="Hit",         //26
	QuidCommentNames(27)="HitNearDeath",         //27
	QuidCommentNames(28)="HitDying",         //28
	QuidCommentNames(29)="Dying",         //29
	QuidCommentNames(30)="Dead",         //30
	QuidCommentNames(31)="ReturnToFlight",         //31
	QuidCommentNames(32)="KeeperDives",         //32
	QuidCommentNames(33)="Block",         //33
	QuidCommentNames(34)="Foul",         //34
	QuidCommentNames(35)="Negative",         //35
	QuidCommentNames(36)="Positive",         //36
	QuidCommentNames(37)="SigningOff",         //37
	QuidCommentNames(38)="InTrenchRun",         //38
	QuidCommentNames(39)="IdleClean",         //39
	QuidCommentNames(40)="IdleRough",         //40
	QuidCommentNames(41)="IdleNoFalls",         //41
	QuidCommentNames(42)="IdleTrickySnitch",         //42

}
