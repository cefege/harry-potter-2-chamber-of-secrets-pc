class Narrator expands baseNarrator;


function LoadDialogObject()
{
	if(CurDialog!=None)
		CurDialog.destroy();

	switch(CurLanguageName)
		{
		case "SPANISH":
			CurDialog=spawn(class 'SpaDialog');
			CurLanguageName="SPANISH";
			break;
		case "GERMAN":
			CurDialog=spawn(class 'GerDialog');
			CurLanguageName="GERMAN";
			break;
		case "PORTUGUESE":
			CurDialog=spawn(class 'PorDialog');
			CurLanguageName="PORTUGUESE";
			break;
		case "FRENCH":
			CurDialog=spawn(class 'FreDialog');
			CurLanguageName="FRENCH";
			break;
		case "ITALIAN":
			CurDialog=spawn(class 'ItaDialog');
			CurLanguageName="ITALIAN";
			break;
		case "ENGLISH":
		default:
			CurDialog=spawn(class 'EngDialog');
			CurEmotes=spawn(class 'EngEmote');
			CurLanguageName="ENGLISH";
			break;
		}

}
 