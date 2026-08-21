//===============================================================================
// #73	Mopsus
//===============================================================================

class  WCMopsus extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardMopsusTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Mopsussmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardMopsusBigTexture FILE=TEXTURES\menu\Folio\Cards\Mopsusbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Mopsus";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=73
    skin(0)=Texture'HProps.skins.WizardCardMopsusTex0'
	textureBig=Texture'WizCardMopsusBigTexture'
	strDescriptionId="WizCard_0078"
}
