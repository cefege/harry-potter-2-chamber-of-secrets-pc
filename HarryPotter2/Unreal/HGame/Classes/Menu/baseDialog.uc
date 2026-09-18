class baseDialog expands info;

var Harry playerHarry;
var int numLines;

const MAX_LINES =  2100;

var string lineIDs[2100];
var sound lineSounds[2100];
var string lineText[2100];
var string LanguageName;

var globalconfig string LanguageExtension;



event Spawned()
{
	if(playerHarry==None)
		foreach AllActors(class'Harry', playerHarry)
			break;
}

function bool FindDialog(string dialogID,out sound dlgSound,out string dlgText)
{
local int i;
	//playerHarry.clientMessage("*******************Looking for dialogID:"$dialogID);

		//check to see if it is an emote first.
/*	dlgsound = Sound( DynamicLoadObject("AllEmote."$dialogID, class'Sound') );
	if(dlgSound!=None)
		{
		dlgText="";
		return(true);
		}
*/	
	dlgText=Localize( "all", dialogID,"HPdialog" );

	//	dlgText = HandleFacialExpression

//	if (!playerharry.bDisableDialog)
		dlgsound = Sound( DynamicLoadObject("AllDialog."$dialogID, class'Sound') );
	if(dlgSound==None)
		{
//		dlgsound = Sound( DynamicLoadObject("AllEmote."$dialogID, class'Sound') );
//		if(dlgSound==None)
			dlgText="*" $dlgText;
		}
	return(true);

}

//**********************************************************************************************
function string TranslateString(string s)
{
	//If it matches one of our keywords, find the translation.  This is all very hard coded
	//if( s ~= "Executive Producers" )

	return s;
}

//**********************************************************************************************
function float DeliverDialog(string dialogID)
{
local float duration;
local sound dlgSound;
local string dlgText;


		//check to see if it is an emote first.
/*	dlgsound = Sound( DynamicLoadObject("AllEmote."$dialogID, class'Sound') );
	if(dlgSound!=None)
		{
		duration=GetSoundDuration(dlgSound);
		duration+=0.5;		//a little space between sounds.
		PlaySound(dlgSound,SLOT_Interact, 3.2, false, 20000.0, 1.0);
		return(duration);
		}
*/
	duration=3.0;
	/*
	if(playerHarry.theNarrator.FindDialog(dialogID,dlgSound,dlgText))
		{
		if(dlgSound!=None)
			{
			duration=GetSoundDuration(dlgSound);
			duration+=0.5;		//a little space between sounds.
			playerHarry.PlaySound(dlgSound,SLOT_Interact, 3.2, false, 20000.0, 1.0);
			}
		if(instr(caps(dialogID),"EMOTIV")==-1)		//see if it is an emote type
			playerHarry.ReceiveIconMessage(None,dlgText,duration);
		}
	else
		playerHarry.ReceiveIconMessage(None,dlgText,duration);
	*/

//playerHarry.clientMessage("*****DeliverDialog:"$dialogID $" " $dlgSound $" " $dlgText);
	return(duration);
}

	//same as DeliverDialog except it doesnt use any text.
function float DeliverEmote(string dialogID)
{
local float duration;
local sound dlgSound;
local string dlgText;
	
	duration=1.0;
	if(FindDialog(dialogID,dlgSound,dlgText))
		{
		if(dlgSound!=None)
			{
			duration=GetSoundDuration(dlgSound);
			duration+=0.5;		//a little space between sounds.
			playerHarry.PlaySound(dlgSound,SLOT_Interact, 3.2, false, 20000.0, 1.0);
			}
		}
	else
		playerHarry.clientMessage("*****DeliverEmote cant find emote:"$dialogID );
	return(duration);
}

defaultproperties
{
numLines=1;
LanguageName="base";
lineIDs(0)="TUT1_DUMINTRO_1")
lineSounds(0)=HPSounds.tut1_dlg.TUT1_DUMINTRO_1
lineText(0)="Welcome to Hogwarts, the school for Witches and Wizards. I am Albus Dumbledore, your Headmaster."

}