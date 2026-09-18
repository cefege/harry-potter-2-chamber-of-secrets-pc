//===============================================================================
// #60	Laverne de Montmorency
//===============================================================================

class  WCMontmorency extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardMontmorencyTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Montmorencysmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardMontmorencyBigTexture FILE=TEXTURES\menu\Folio\Cards\Montmorencybig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Laverne de Montmorency";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=60
    skin(0)=Texture'HProps.skins.WizardCardMontmorencyTex0'
	textureBig=Texture'WizCardMontmorencyBigTexture'
	strDescriptionId="WizCard_0096"
}
