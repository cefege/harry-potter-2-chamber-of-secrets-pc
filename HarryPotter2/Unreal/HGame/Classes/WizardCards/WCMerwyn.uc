//===============================================================================
// #12 Merwyn the Malicious
//===============================================================================

class  WCMerwyn extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardMerwynTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Merwynsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardMerwynBigTexture FILE=TEXTURES\menu\Folio\Cards\Merwynbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Merwyn the Malicious";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=12
    skin(0)=Texture'HProps.skins.WizardCardMerwynTex0'
	textureBig=Texture'WizCardMerwynBigTexture'
	strDescriptionId="WizCard_0050"
}
