class CutScriptDisk expands CutScript;

//var array<string> lineArray;
var string lineArray[4096];
var int curScriptLine;

function bool GetNextLine(out string line)
{
local int xx;

//	xx=lineArray.len();

//	if(curScriptLine>ArrayCount(lineArray)
//		return(false);	//past end of script.

	line=lineArray[curScriptLine];
	curScriptLine++;

	if(line=="")
		return false;	//end of array.

	return(true);
}

function load(string threadName,string fileName)
{
local int i;
local string line;

	for(i=0;i<9999;i++)	//note lots of potential lines
		{
		line=Localize( threadName, "line_"$i,fileName );
		if(line=="" || instr(line,"<?")>-1)	
			break;	//no more lines.
		lineArray[i]=line;
		}
}
