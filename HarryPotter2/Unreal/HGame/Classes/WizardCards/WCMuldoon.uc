//===============================================================================
// #10 Burdock Muldoon
//===============================================================================

class  WCMuldoon extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardMuldoonTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Muldoonsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardMuldoonBigTexture FILE=TEXTURES\menu\Folio\Cards\Muldoonbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Burdock Muldoon";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=10
    skin(0)=Texture'HProps.skins.WizardCardMuldoonTex0'
	textureBig=Texture'WizCardMuldoonBigTexture'
	strDescriptionId="WizCard_0037"
}
