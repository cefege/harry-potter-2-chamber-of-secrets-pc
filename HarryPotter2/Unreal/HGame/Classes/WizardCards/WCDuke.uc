//===============================================================================
// #68	Kirley Duke
//===============================================================================

class  WCDuke extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardDukeTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Dukesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardDukeBigTexture FILE=TEXTURES\menu\Folio\Cards\Dukebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Kirley Duke";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=68
    skin(0)=Texture'HProps.skins.WizardCardDukeTex0'
	textureBig=Texture'WizCardDukeBigTexture'
	strDescriptionId="WizCard_0062"
}
