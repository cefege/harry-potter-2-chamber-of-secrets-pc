//===============================================================================
//  #77 Norvel Twonk
//===============================================================================

class  WCTwonk extends bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardTwonkTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Twonksmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardTwonkBigTexture FILE=TEXTURES\menu\Folio\Cards\Twonkbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


function PostBeginPlay()
{
	WizardName = "Norvel Twonk";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=77
    skin(0)=Texture'HProps.skins.WizardCardTwonkTex0'
	textureBig=Texture'WizCardTwonkBigTexture'
	strDescriptionId="WizCard_0071"
}
