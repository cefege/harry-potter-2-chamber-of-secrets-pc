//===============================================================================
// #39	Gwenog Jones
//===============================================================================

class  WCJones extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardJonesTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Jonessmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardJonesBigTexture FILE=TEXTURES\menu\Folio\Cards\Jonesbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Gwenog Jones";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=39
    skin(0)=Texture'HProps.skins.WizardCardJonesTex0'
	textureBig=Texture'WizCardJonesBigTexture'
	strDescriptionId="WizCard_0059"
}
