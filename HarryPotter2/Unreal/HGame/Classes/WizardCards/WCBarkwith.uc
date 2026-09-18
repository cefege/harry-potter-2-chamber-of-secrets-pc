//===============================================================================
//  #50 Musidora Barkwith
//===============================================================================

class  WCBarkwith extends bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardBarkwithTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Barkwithsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardBarkwithBigTexture FILE=TEXTURES\menu\Folio\Cards\Barkwithbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Musidora Barkwith";	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=50
    skin(0)=Texture'HProps.skins.WizardCardBarkwithTex0'
	textureBig=Texture'WizCardBarkwithBigTexture'
	strDescriptionId="WizCard_0008"
}
