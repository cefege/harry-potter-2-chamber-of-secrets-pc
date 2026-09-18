//===============================================================================
//  #8 Derwent Shimpling
//===============================================================================

class  WCShimpling extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardShimplingTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Shimplingsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardShimplingBigTexture FILE=TEXTURES\menu\Folio\Cards\Shimplingbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Derwent Shimpling";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=8
    skin(0)=Texture'HProps.skins.WizardCardShimplingTex0'
	textureBig=Texture'WizCardShimplingBigTexture'
	strDescriptionId="WizCard_0057"
}
