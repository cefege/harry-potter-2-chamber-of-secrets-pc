
//Yet another file by me, which does nothing, and is full of commented out code.

class StoryBookDialog expands baseDialog;

/*
struct DialogLine
{
	var string name;
	var sound sound;
	var string text;
};
*/

/*
const MAX_LINES =  20;
//var string DlgLineNames[200];
var int   DlgLineBook[20];
var int   DlgLinePage[20];
var sound DlgLineSounds[20];
var string DlgLineText[20];
*/
var baseNarrator _narrator;

//***********************************************************************************************************
function bool FindDialog2(string PageName, /*int Book, int Page,*/ out sound dlgSound, out float SoundLen, out string dlgText)
{
	local int  i;
	local bool bReturnVal;
	//local bool bFakeTut1Dialog;

	bReturnVal = true;
	//bFakeTut1Dialog = false;

	if( _narrator == none )
	{
		foreach AllActors(class'baseNarrator', _narrator)
			break;

		if( _narrator == None )
		{
			Log("*********** No Tut1Dialog found");

			//_narrator = spawn(class'narrator');
			//bFakeTut1Dialog = true;
		}
	}

	_narrator.FindDialog( PageName, dlgSound, dlgText );

/*
	     if( PageName ~= "CommonRoom" )       _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "HarryGetsBroom" )   _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "HarryStudy" )       _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "3_1_" )             _narrator.FindDialog( "StoryBook1", dlgSound, dlgText );
	else if( PageName ~= "3_2_" )             _narrator.FindDialog( "StoryBook2", dlgSound, dlgText );
	else if( PageName ~= "3_3_" )             _narrator.FindDialog( "StoryBook3", dlgSound, dlgText );
	else if( PageName ~= "3_4_" )             _narrator.FindDialog( "StoryBook4", dlgSound, dlgText );
	else if( PageName ~= "3_5_" )             _narrator.FindDialog( "StoryBook5", dlgSound, dlgText );
	else if( PageName ~= "3_6_" )             _narrator.FindDialog( "StoryBook6", dlgSound, dlgText );
	else if( PageName ~= "3_7_" )             _narrator.FindDialog( "StoryBook7", dlgSound, dlgText );
	else if( PageName ~= "4_1_" )             _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "4_2_" )             _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "HermioneLibrary" )  _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "5_1_" )             _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "5_2_" )             _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "5_3_" )             _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "5_4_" )             _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "5_6_" )             _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "6_1_" )             _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "6_2_" )             _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "6_3_" )             _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "6_4_" )             _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "6_5_" )             _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "6_6_" )             _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "6_7_" )             _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "6_8_" )             _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "7_1_" )             _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "QuidditchVictory" ) _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else if( PageName ~= "RunDownHall" )      _narrator.FindDialog( "StoryBook8", dlgSound, dlgText );
	else bReturnVal = false;
*/
	if( dlgSound != none )
		SoundLen = GetsoundDuration(dlgSound);
	else
		SoundLen = 2;

	//switch( Book )
	//{
	//	case 0:  //intro dialog
	//		Page = Clamp( Page, 0, 7 );
	//		_narrator.FindDialog( "StoryBook" $ (Page+1), dlgSound, dlgText );
	//		SoundLen = GetsoundDuration(dlgSound);
	//		bReturnVal = true;
	//		break;
	//
	//	case 1:  // ?
	//		//Page = Clamp( Page, 0, 7 );
	//		Page = 0;
	//		//_narrator.FindDialog( "StoryBook" $ (Page+1), dlgSound, dlgText );
	//		_narrator.FindDialog( "StoryBook9", dlgSound, dlgText );
	//		SoundLen = GetsoundDuration(dlgSound);
	//		bReturnVal = true;
	//		break;
	//
	//	case 2:  // ?
	//		Page = Clamp( Page, 0, 7 );
	//		Page = 0;
	//		//_narrator.FindDialog( "StoryBook" $ (Page+1), dlgSound, dlgText );
	//		_narrator.FindDialog( "StoryBook10", dlgSound, dlgText );
	//		SoundLen = GetsoundDuration(dlgSound);
	//		bReturnVal = true;
	//		break;
	//}

	/*
	//log("Looking for " $tstr);
	for( i = 0; i < MAX_LINES; i++ )
	{
		//log("Checking " $Tut1DlgLineNames[i] $"->" $Tut1DlgLineText[i]);
		if( DlgLineBook[ i ] == Book  &&  DlgLinePage[ i ] == Page )
		{
			//log("Found " $lineName);
			dlgSound = DlgLineSounds[i];
			dlgText = DlgLineText[i];

			SoundLen = GetsoundDuration(dlgSound);

			return(true);
		}
	} 

	dlgSound=None; 
	//	dlgText="Cant find Dialog called:" $lineName;
	//dlgText=":" $lineName;
	*/

	//if( bFakeTut1Dialog )
	//	_Tut1Dialog.Destroy();

	return bReturnVal;
}

/*
	DlgLineBook(0)=0
	DlgLinePage(0)=0
	DlgLineSounds(0)=HPSounds.tut1_dlg.TUT1_DUMINTRO_1
	DlgLineText(0)="Welcome to Hogwarts, the school for Witches and Wizards. I am Albus Dumbledore, your Headmaster."

	DlgLineBook(1)=0
	DlgLinePage(1)=1
	DlgLineSounds(1)=HPSounds.tut1_dlg.TUT1_DUMINTRO_2
	DlgLineText(1)="Hogwarts is full of secrets, Harry, so search behind every."

	DlgLineBook(2)=0
	DlgLinePage(2)=2
	DlgLineSounds(2)=HPSounds.tut1_dlg.TUT1_DUMINTRO_2
	DlgLineText(2)="Hogwarts is full of secrets, Harry, so search behind every door."

	DlgLineBook(3)=0
	DlgLinePage(3)=3
	DlgLineSounds(3)=HPSounds.tut1_dlg.TUT1_DUMINTRO_2
	DlgLineText(3)="Hogwarts is full of secrets, Harry, so search behind every door.  But."


	DlgLineBook(3)=1
	DlgLinePage(3)=0
	DlgLineSounds(3)=HPSounds.tut1_dlg.TUT1_DUMINTRO_2
	DlgLineText(3)="Hogwarts is full of secrets, Harry, so search behind every door.  But."
*/

defaultProperties
{
}