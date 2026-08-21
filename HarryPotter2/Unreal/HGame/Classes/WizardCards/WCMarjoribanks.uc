//===============================================================================
// #33 Beaumont Marjoribanks
//===============================================================================

class  WCMarjoribanks extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardMarjoribanksTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Marjoribankssmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardMarjoribanksBigTexture FILE=TEXTURES\menu\Folio\Cards\Marjoribanksbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Beaumont Marjoribanks";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=33
    skin(0)=Texture'HProps.skins.WizardCardMarjoribanksTex0'
	textureBig=Texture'WizCardMarjoribanksBigTexture'
	strDescriptionId="WizCard_0029"
}
