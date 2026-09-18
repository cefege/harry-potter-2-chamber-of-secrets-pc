class Tut1Dialog expands baseDialog;

struct DialogLine
{
	var string name;
	var sound sound;
	var string text;
};

const MAX_LINES =  200;

var string Tut1DlgLineNames[200];
var sound Tut1DlgLineSounds[200];
var string Tut1DlgLineText[200];

function bool FindDialog(string lineName,out sound dlgSound,out string dlgText)
{
local int i;
local string tstr;
	tstr=caps(lineName);

//log("Looking for " $tstr);
	for(i=0;i<MAX_LINES;i++)
		{
//log("Checking " $Tut1DlgLineNames[i] $"->" $Tut1DlgLineText[i]);
		if(Tut1DlgLineNames[i]==tstr)
			{
			log("Found " $lineName);
			dlgSound=Tut1DlgLineSounds[i];
			dlgText=Tut1DlgLineText[i];

			return(true);
			}
		} 

	dlgSound=None; 
//	dlgText="Cant find Dialog called:" $lineName;
	dlgText=":" $lineName;
	return(false);
}

defaultProperties
{

}