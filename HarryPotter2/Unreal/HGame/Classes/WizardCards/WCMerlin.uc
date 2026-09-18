//===============================================================================
//  #1 Merlin
//===============================================================================

class  WCMerlin extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardMerlinTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Merlinsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardMerlinBigTexture FILE=TEXTURES\menu\Folio\Cards\merlinbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Merlin";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=1
    skin(0)=Texture'HProps.skins.WizardCardMerlinTex0'
	textureBig=Texture'WizCardMerlinBigTexture'
	strDescriptionId="WizCard_0039"
}
